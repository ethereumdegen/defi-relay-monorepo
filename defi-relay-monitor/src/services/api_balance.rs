use crate::config::Config;
use reqwest::Client;
use serde_json::Value;

pub struct ApiBalance {
    pub name: String,
    pub result: Result<String, String>,
}

pub async fn fetch_api_balances(client: &Client, config: &Config) -> Vec<ApiBalance> {
    let mut futs: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ApiBalance> + Send>>> =
        Vec::new();

    if let Some(key) = &config.moonshot_api_key {
        let client = client.clone();
        let key = key.clone();
        futs.push(Box::pin(async move { fetch_moonshot(&client, &key).await }));
    }

    futures::future::join_all(futs).await
}

async fn fetch_moonshot(client: &Client, api_key: &str) -> ApiBalance {
    let name = "MoonshotAI".to_string();
    let result = do_fetch_moonshot(client, api_key).await;
    ApiBalance { name, result }
}

async fn do_fetch_moonshot(client: &Client, api_key: &str) -> Result<String, String> {
    let resp = client
        .get("https://api.moonshot.ai/v1/users/me/balance")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = resp.status();
    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse response: {e}"))?;

    if !status.is_success() {
        return Err(format!("HTTP {status}: {json}"));
    }

    // MoonshotAI returns balance in data.available_balance or data.balance
    let data = json.get("data").unwrap_or(&json);
    if let Some(balance) = data.get("available_balance").and_then(|v| v.as_f64()) {
        return Ok(format!("¥{:.2} remaining", balance));
    }
    if let Some(balance) = data.get("balance").and_then(|v| v.as_f64()) {
        return Ok(format!("¥{:.2} remaining", balance));
    }
    if let Some(cash) = data.get("cash_balance") {
        return Ok(format!("¥{} remaining", cash));
    }

    Err(format!("unexpected response format: {json}"))
}
