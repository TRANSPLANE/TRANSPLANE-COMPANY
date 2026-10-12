use super::*;

fn preview_type(name: &str) -> (&'static str, &'static str) {
    match Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "markdown" => ("markdown", "text/markdown; charset=utf-8"),
        "txt" | "csv" | "json" | "log" | "yaml" | "yml" | "toml" => {
            ("text", "text/plain; charset=utf-8")
        }
        "png" => ("image", "image/png"),
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "gif" => ("image", "image/gif"),
        "webp" => ("image", "image/webp"),
        "pdf" => ("pdf", "application/pdf"),
        _ => ("unsupported", "application/octet-stream"),
    }
}
fn valid_content(bytes: &[u8], mime: &str) -> bool {
    match mime {
        "image/png" => {
            bytes.len() >= 24
                && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                && &bytes[12..16] == b"IHDR"
        }
        "image/jpeg" => bytes.len() >= 4 && bytes.starts_with(b"\xff\xd8\xff"),
        "image/gif" => {
            bytes.len() >= 10 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"))
        }
        "image/webp" => bytes.len() >= 16 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "application/pdf" => bytes.starts_with(b"%PDF-"),
        _ => std::str::from_utf8(bytes).is_ok() && !bytes.contains(&0),
    }
}

impl App {
    pub async fn file_preview(
        &self,
        company: &str,
        id: &str,
        revision: Option<i64>,
    ) -> Result<Value> {
        self.company(company).await?;
        identifier(id)?;
        let statement = if revision.is_some() {
            "SELECT data FROM record_versions WHERE company_id=? AND kind='file' AND id=? AND revision=?"
        } else {
            "SELECT data FROM records WHERE company_id=? AND kind='file' AND id=?"
        };
        let mut q = query!(&sql(self.pg, statement)).bind(company).bind(id);
        if let Some(revision) = revision {
            q = q.bind(revision);
        }
        let row = q
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| err("not_found", "文件版本不存在"))?;
        let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
        let name = Path::new(text(&record.data, "relative_path")?)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let (format, mime) = preview_type(&name);
        let size = integer(&record.data, "size_bytes")?;
        let limit = if ["markdown", "text"].contains(&format) {
            1024 * 1024
        } else {
            10 * 1024 * 1024
        };
        let base = format!("/api/v1/companies/{company}/files/{id}");
        let mut result = json!({"file_id":id,"revision":record.revision,"name":name,"size_bytes":size,"sha256":record.data["sha256"],"format":format,"mime_type":mime,"previewable":false,"download_url":format!("{base}?revision={}",record.revision)});
        if format == "unsupported" {
            result["reason"] = json!("unsupported_format");
            return Ok(result);
        }
        if size > limit || size < 0 {
            result["reason"] = json!("too_large");
            return Ok(result);
        }
        let digest = text(&record.data, "sha256")?;
        let object = self.root.join("objects").join(digest);
        if std::fs::metadata(&object)
            .map_err(|_| err("integrity_error", "文件内容缺失"))?
            .len()
            > limit as u64
        {
            return Err(err("integrity_error", "文件大小与登记不符"));
        }
        let (bytes, _) = self
            .download_file(company, id, Some(record.revision))
            .await?;
        if bytes.len() as i64 != size {
            return Err(err("integrity_error", "文件大小与登记不符"));
        }
        if !valid_content(&bytes, mime) {
            result["reason"] = json!("invalid_content");
            return Ok(result);
        }
        result["previewable"] = json!(true);
        if ["markdown", "text"].contains(&format) {
            result["content_text"] =
                json!(std::str::from_utf8(&bytes)?.trim_start_matches('\u{feff}'));
        } else {
            result["content_url"] = json!(format!(
                "{base}/preview/content?revision={}",
                record.revision
            ));
        }
        Ok(result)
    }
    pub async fn file_preview_content(
        &self,
        company: &str,
        id: &str,
        revision: Option<i64>,
    ) -> Result<(Vec<u8>, String)> {
        let metadata = self.file_preview(company, id, revision).await?;
        if metadata["previewable"] != true
            || ![json!("image"), json!("pdf")].contains(&metadata["format"])
        {
            return Err(err("unsupported_preview", "此文件只提供原件下载"));
        }
        let (bytes, _) = self
            .download_file(company, id, Some(integer(&metadata, "revision")?))
            .await?;
        Ok((bytes, text(&metadata, "mime_type")?.to_owned()))
    }
}
