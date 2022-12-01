use std::collections::HashMap;

use reqwest::{ blocking::Client, Method };
use anyhow::Result;
use super::{ Config, ReportDetails };

pub fn get_billable_report(config: &Config, client_name: &str) -> Result<ReportDetails> {
    let client = &config.clients[client_name];
    let url = "https://api.track.toggl.com/reports/api/v2/details";

    let mut req_query: HashMap<&str, &str> = HashMap::new();
    req_query.insert("client_ids", &client.id);
    req_query.insert("since", &client.last_billed_date);

    make_request(Method::GET, url, req_query, config)
        .and_then(|res| serde_json::from_str::<ReportDetails>(&res).map_err(|e| anyhow::anyhow!(e)))
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
