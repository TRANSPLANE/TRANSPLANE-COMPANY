use super::*;

impl App {
    /// A read transaction makes graph entities, attribution and the displayed company version coherent.
    pub async fn network(
        &self,
        company_id: &str,
        period_start: Option<&str>,
        period_end: Option<&str>,
        basis: Option<&str>,
    ) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        if self.pg {
            query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                .execute(&mut *tx)
                .await?;
        }
        let company = self.company_tx(&mut tx, company_id).await?;
        let rows = query!(&sql(self.pg,"SELECT data FROM records WHERE company_id=? AND kind IN ('customer','business','resource','subscription','network_entity','relationship','profit_entry') ORDER BY kind,id"))
            .bind(company_id).fetch_all(&mut *tx).await?;
        let records = rows
            .into_iter()
            .map(|row| {
                Ok(serde_json::from_str::<Record>(
                    &row.try_get::<String, _>("data")?,
                )?)
            })
            .collect::<Result<Vec<_>>>()?;
        let as_of = company.simulation_date.clone().unwrap_or_else(|| {
            Utc::now()
                .with_timezone(
                    &company
                        .timezone
                        .parse::<chrono_tz::Tz>()
                        .expect("validated timezone"),
                )
                .date_naive()
                .to_string()
        });
        let graph = company_core::network::projection(
            &company,
            &records,
            &as_of,
            period_start,
            period_end,
            basis,
        )?;
        tx.commit().await?;
        Ok(graph)
    }

    pub(super) async fn network_operation_tx(
        &self,
        tx: &mut Transaction<'_, Any>,
        company: &Company,
        existing: &[Record],
        operation: &str,
        request: &OperationRequest,
    ) -> Result<Record> {
        let kind = operation
            .strip_suffix(".put")
            .filter(|kind| company_core::network::RECORD_KINDS.contains(kind))
            .ok_or_else(|| err("unknown_operation", "关系网操作不存在"))?;
        let id = text(&request.input, "id")?;
        identifier(id)?;
        if request.input.get("fork_provenance").is_some() {
            return Err(err("invalid_input", "关系网记录包含系统维护字段"));
        }
        let mut data = request.input.clone();
        let old = self.record_tx(tx, &company.id, kind, id).await?;
        if data.get("status").is_none() {
            data["status"] = json!("active");
        }
        if data.get("source_refs").is_none() && kind != "profit_entry" {
            data["source_refs"] = json!([]);
        }
        if let Some(origin) = old.as_ref().and_then(|r| r.data.get("fork_provenance")) {
            data["fork_provenance"] = origin.clone();
        }
        company_core::network::validate_record(kind, &data, company, existing)?;
        for reference in data["source_refs"].as_array().into_iter().flatten() {
            let found = query!(&sql(self.pg,"SELECT revision FROM record_versions WHERE company_id=? AND kind=? AND id=? AND revision=?"))
                .bind(&company.id).bind(text(reference,"kind")?).bind(text(reference,"id")?).bind(integer(reference,"revision")?).fetch_optional(&mut **tx).await?;
            if found.is_none() {
                return Err(err(
                    "reference_not_found",
                    "关系网来源的准确版本不存在于当前公司",
                ));
            }
        }
        self.save_record(
            tx,
            &company.id,
            kind,
            id,
            data,
            request.expected_revision,
            company.revision + 1,
        )
        .await
    }
}
