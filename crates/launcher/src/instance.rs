use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Existing {
    pub url: String,
    marker: String,
}

pub enum Acquisition {
    Acquired(Instance),
    Existing(Existing),
}

pub struct Instance {
    _lock: File,
    state_path: PathBuf,
    marker: String,
}

pub fn data_directory(executable: &Path, requested: Option<&Path>) -> Result<PathBuf> {
    match requested {
        Some(path) => Ok(path.to_owned()),
        None => Ok(executable
            .parent()
            .context("Executable has no parent folder")?
            .join("company-data")),
    }
}

pub fn browser_url(address: SocketAddr) -> String {
    let ip = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    format!("http://{}", SocketAddr::new(ip, address.port()))
}

impl Instance {
    pub fn acquire(data_dir: &Path) -> Result<Acquisition> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(data_dir.join("config/launcher.lock"))?;
        let state_path = data_dir.join("config/launcher-state.json");
        match file.try_lock() {
            Ok(()) => Ok(Acquisition::Acquired(Self {
                _lock: file,
                state_path,
                marker: uuid::Uuid::new_v4().to_string(),
            })),
            Err(std::fs::TryLockError::WouldBlock) => {
                // Windows byte-range locks also block reads of the locked file.
                // Keep public instance metadata in a separate, credential-free file.
                let content = std::fs::read_to_string(state_path).context(
                    "This company deployment is already starting. Try again after its window opens.",
                )?;
                let existing: Existing = serde_json::from_str(&content)
                    .context("This company deployment is already starting. Try again after its window opens.")?;
                existing.verify()?;
                Ok(Acquisition::Existing(existing))
            }
            Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
        }
    }

    pub fn marker(&self) -> &str {
        &self.marker
    }

    pub fn publish(&mut self, url: &str) -> Result<()> {
        let state = Existing {
            url: url.into(),
            marker: self.marker.clone(),
        };
        let mut state_file = File::create(&self.state_path)?;
        serde_json::to_writer(&mut state_file, &state)?;
        state_file.flush()?;
        state_file.sync_data()?;
        Ok(())
    }
}

impl Existing {
    fn verify(&self) -> Result<()> {
        let address: SocketAddr = self
            .url
            .strip_prefix("http://")
            .context("Invalid running launcher address")?
            .parse()?;
        // A random per-process marker prevents a stale lock file from opening an unrelated server.
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
            .context("The company window is running, but its service is not ready")?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        write!(
            stream,
            "GET /_company-launcher/{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.marker, address
        )?;
        let mut response = String::new();
        stream.take(8192).read_to_string(&mut response)?;
        let Some((headers, body)) = response.split_once("\r\n\r\n") else {
            bail!("The running company service could not be verified");
        };
        if !headers.starts_with("HTTP/1.1 200 ") || body != self.marker {
            bail!("This address is not the running company deployment");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_data_stays_beside_the_executable() {
        assert_eq!(
            data_directory(Path::new("/portable/company-launch"), None).unwrap(),
            Path::new("/portable/company-data")
        );
        assert_eq!(
            data_directory(
                Path::new("/portable/company-launch"),
                Some(Path::new("/other/data"))
            )
            .unwrap(),
            Path::new("/other/data")
        );
    }

    #[test]
    fn wildcard_bind_opens_a_loopback_url_without_credentials() {
        assert_eq!(
            browser_url("0.0.0.0:4188".parse().unwrap()),
            "http://127.0.0.1:4188"
        );
        assert_eq!(
            browser_url("[::]:4188".parse().unwrap()),
            "http://[::1]:4188"
        );
    }

    #[test]
    fn stale_unlocked_state_does_not_block_a_new_service() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("config")).unwrap();
        {
            let Acquisition::Acquired(mut first) = Instance::acquire(temp.path()).unwrap() else {
                panic!("lock not acquired")
            };
            first.publish("http://127.0.0.1:1").unwrap();
        }
        assert!(matches!(
            Instance::acquire(temp.path()).unwrap(),
            Acquisition::Acquired(_)
        ));
    }

    #[test]
    fn stale_metadata_never_accepts_another_http_service() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let existing = Existing {
            url: format!("http://{}", listener.local_addr().unwrap()),
            marker: "expected-instance".into(),
        };
        let responder = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nother",
                )
                .unwrap();
        });
        assert!(existing.verify().is_err());
        responder.join().unwrap();
    }
}
