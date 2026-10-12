use super::*;

impl App {
    async fn register_fork_file_contents_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        fork: &Company,
        source: &Record,
    ) -> Result<(Value, Value)> {
        let first_digest = text(&source.data, "sha256")?.to_owned();
        let first_size = integer(&source.data, "size_bytes")?;
        let mut current = source.clone();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..16 {
            if current.kind != "file" {
                break;
            }
            let digest = text(&current.data, "sha256")?.to_owned();
            if !seen.insert(digest.clone()) {
                return Err(err("integrity_error", "推演基线依据来源不符"));
            }
            let bytes = std::fs::read(self.root.join("objects").join(&digest))?;
            if hash(&bytes) != digest || bytes.len() as i64 != integer(&current.data, "size_bytes")?
            {
                return Err(err("integrity_error", "推演来源文件摘要或大小不符"));
            }
            let id = format!("fork-source-{digest}");
            if self.record_tx(tx, &fork.id, "file", &id).await?.is_none() {
                self.save_record(
                    tx,
                    &fork.id,
                    "file",
                    &id,
                    json!({"id":id,"title":"推演文件原件",
                    "relative_path":format!("extensions/provenance/{id}.bin"),"category":"context",
                    "sha256":digest,"size_bytes":bytes.len(),"fork_source_content":true}),
                    None,
                    1,
                )
                .await?;
            }
            if current.data["fork_evidence"] != true {
                break;
            }
            let proof: Value = serde_json::from_slice(&bytes)?;
            if let Some(encoded) = proof["source_file"]["content_base64"].as_str() {
                let original = STANDARD.decode(encoded)?;
                let restored_digest = self.write_object(&original)?;
                if proof["source_file"]["sha256"] != restored_digest
                    || proof["source_file"]["size_bytes"] != original.len() as i64
                {
                    return Err(err("integrity_error", "推演基线依据摘要或大小不符"));
                }
            }
            current = serde_json::from_value(proof["source_record"].clone())?;
        }
        if current.kind == "file" && current.data["fork_evidence"] == true {
            return Err(err("capacity_exceeded", "推演基线依据嵌套过深"));
        }
        Ok((
            json!({"sha256":first_digest,"size_bytes":first_size,"file_ref":{"kind":"file","id":format!("fork-source-{first_digest}"),"revision":1}}),
            json!({"company_id":current.company_id,"kind":current.kind,"id":current.id,"revision":current.revision,"company_revision":current.company_revision}),
        ))
    }
    /// Reference evidence keeps its exact original revision when the source history
    /// is not copied into the independent simulation company.
    async fn fork_reference_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        parent: &Company,
        fork: &Company,
        baseline: &Value,
        reference: &Value,
    ) -> Result<Value> {
        let kind = text(reference, "kind")?;
        let id = text(reference, "id")?;
        let revision = integer(reference, "revision")?;
        let proof_id = format!(
            "fork-proof-{}",
            hash(format!("{}:{kind}:{id}:{revision}", parent.id).as_bytes())
        );
        if self
            .record_tx(tx, &fork.id, "file", &proof_id)
            .await?
            .is_none()
        {
            let row = query!(&sql(self.pg,"SELECT data FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                .bind(&parent.id).bind(kind).bind(id).bind(revision).fetch_optional(&mut **tx).await?
                .ok_or_else(|| err("reference_not_found", "推演来源的准确历史版本不存在"))?;
            let source: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if source.company_revision > parent.revision {
                return Err(err("integrity_error", "推演来源晚于公司基线"));
            }
            let mut proof = json!({"format":"transplane-company-fork-evidence","version":1,
                "origin":{"company_id":parent.id,"kind":kind,"id":id,"revision":revision,"company_revision":source.company_revision},
                "source_record":source,"baseline":{"company_id":parent.id,"company_revision":parent.revision,"metrics":baseline}});
            let mut evidence_origin = proof["origin"].clone();
            if source.kind == "file" {
                let (file, origin) = self
                    .register_fork_file_contents_tx(tx, fork, &source)
                    .await?;
                proof["source_file"] = file;
                evidence_origin = origin;
            }
            let bytes = serde_json::to_vec(&proof)?;
            let digest = self.write_object(&bytes)?;
            self.save_record(tx,&fork.id,"file",&proof_id,
                json!({"id":proof_id,"title":"推演基线依据","relative_path":format!("extensions/provenance/{proof_id}.json"),
                    "category":"context","sha256":digest,"size_bytes":bytes.len(),"fork_evidence":true,"fork_evidence_origin":evidence_origin}),None,1).await?;
        }
        Ok(json!({"kind":"file","id":proof_id,"revision":1}))
    }

    pub(super) async fn copy_fork_records_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        parent: &Company,
        fork: &Company,
    ) -> Result<()> {
        let baseline = self.metrics_tx(tx, parent, None).await?;
        let rows = query!(&sql(
            self.pg,
            "SELECT data FROM records WHERE company_id=? ORDER BY kind,id"
        ))
        .bind(&parent.id)
        .fetch_all(&mut **tx)
        .await?;
        for row in rows {
            let record: Record = serde_json::from_str(&row.try_get::<String, _>("data")?)?;
            if ["simulation_turn", "forecast", "report", "decision"].contains(&record.kind.as_str())
                || (record.kind == "file"
                    && (record.data["fork_evidence"] == true
                        || record.data["fork_source_content"] == true))
            {
                continue;
            }
            let mut data = record.data.clone();
            if record.kind == "skill" {
                data["fork_provenance"] = json!({"company_id":parent.id,"skill_id":record.id,"revision":record.revision,"source_refs":data.get("source_refs").cloned().unwrap_or(json!([]))});
                data["source_refs"] = json!([]);
            }
            if company_core::network::RECORD_KINDS.contains(&record.kind.as_str())
                || ["quest", "alert"].contains(&record.kind.as_str())
            {
                data["fork_provenance"] = json!({"company_id":parent.id,"kind":record.kind,"id":record.id,"revision":record.revision,"company_revision":record.company_revision,"source_record":record});
                for path in [
                    "/source_refs",
                    "/completion/result_refs",
                    "/resolution/source_refs",
                ] {
                    if let Some(refs) = data.pointer(path).and_then(Value::as_array).cloned() {
                        let mut mapped = Vec::new();
                        for reference in refs {
                            mapped.push(
                                self.fork_reference_tx(tx, parent, fork, &baseline, &reference)
                                    .await?,
                            );
                        }
                        *data.pointer_mut(path).unwrap() = json!(mapped);
                    }
                }
                for field in ["source", "target", "subject"] {
                    if data[field]["kind"] == "company" && data[field]["id"] == parent.id {
                        data[field]["id"] = json!(fork.id);
                    }
                }
                if record.kind == "profit_entry"
                    && data
                        .get("assumptions")
                        .is_none_or(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
                {
                    data["assumptions"] =
                        json!("继承原公司的已登记收支作为独立推演基线；后续模拟结果仅属于此分支。");
                }
                if data["ack_only"] == true {
                    continue;
                }
                if record.kind == "alert" {
                    // Acknowledgements describe a reader in the source company.
                    data.as_object_mut().unwrap().remove("acknowledgement");
                }
            }
            self.save_record(tx, &fork.id, &record.kind, &record.id, data, None, 1)
                .await?;
        }
        Ok(())
    }
}
