use std::collections::HashMap;

use chrono::{Months, NaiveDate};
use reqwest::{ blocking::Client, Method };
use anyhow::Result;
use super::{ Config, ReportDetails };

pub fn get_billable_report(config: &Config, client_name: &str) -> Result<ReportDetails> {
    let client = &config.clients[client_name];
    let url = "https://api.track.toggl.com/reports/api/v2/details";

    let mut since = NaiveDate::parse_from_str(&config.start_of_time, "%Y-%m-%d")?;

    let mut full_report = ReportDetails {
        data: Vec::new(),
    };

    while since < chrono::offset::Local::now().date_naive() {
        let until = since.checked_add_months(Months::new(12)).ok_or_else(|| anyhow::anyhow!("Failed to add 12 months to since date"))?;

        let since_string = since.to_string();
        let until_string = until.to_string();

        let mut req_query: HashMap<&str, &str> = HashMap::new();
        req_query.insert("client_ids", &client.id);
        req_query.insert("since", &since_string);
        req_query.insert("until", &until_string);

        let mut year_report = make_request(Method::GET, url, req_query, config)
            .and_then(|res| serde_json::from_str::<ReportDetails>(&res).map_err(|e| anyhow::anyhow!(e)))?;

        full_report.data.append(&mut year_report.data);

        since = until;
    }

    Ok(full_report)
}

fn make_request(method: Method, url: &str, query_params: HashMap<&str, &str>, config: &Config) -> Result<String> {
    let client = Client::new();

    let token = base64::encode(format!("{}:api_token", std::env::var("TOGGLE_API_TOKEN")?));

    let mut base_params = HashMap::new();
    base_params.insert("user_agent", "toggl-rs");
    base_params.insert("workspace_id", &config.workspace_id);
    
    client.request(method, url)
        .header("Authorization", format!("Basic {}", token))
        .header("Content-Type", "application/json")
        .query(&base_params)
        .query(&query_params)
        .send()
        .and_then(|r| r.text())
        .map_err(|e| anyhow::anyhow!(e))
}
