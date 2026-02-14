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

    if let Some(key) = &config.openai_api_key {
        let client = client.clone();
        let key = key.clone();
        futs.push(Box::pin(async move { fetch_openai(&client, &key).await }));
    }

    if let Some(key) = &config.moonshot_api_key {
        let client = client.clone();
        let key = key.clone();
        futs.push(Box::pin(async move { fetch_moonshot(&client, &key).await }));
    }

    if let Some(key) = &config.minimax_api_key {
        let client = client.clone();
        let key = key.clone();
        let group_id = config.minimax_group_id.clone().unwrap_or_default();
        futs.push(Box::pin(
            async move { fetch_minimax(&client, &key, &group_id).await },
        ));
    }

    futures::future::join_all(futs).await
}

async fn fetch_openai(client: &Client, api_key: &str) -> ApiBalance {
    let name = "OpenAI".to_string();
    let result = do_fetch_openai(client, api_key).await;
    ApiBalance { name, result }
}

async fn do_fetch_openai(client: &Client, api_key: &str) -> Result<String, String> {
    let resp = client
        .get("https://api.openai.com/dashboard/billing/credit_grants")
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
        return Err(format!(
            "HTTP {status}: {}",
            json.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error")
        ));
    }

    let total_available = json
        .get("total_available")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "missing total_available field".to_string())?;

    Ok(format!("${:.2} remaining", total_available))
}

async fn fetch_moonshot(client: &Client, api_key: &str) -> ApiBalance {
    let name = "MoonshotAI".to_string();
    let result = do_fetch_moonshot(client, api_key).await;
    ApiBalance { name, result }
}

async fn do_fetch_moonshot(client: &Client, api_key: &str) -> Result<String, String> {
    let resp = client
        .get("https://api.moonshot.cn/v1/users/me/balance")
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
    // If we can't find a numeric balance, return the raw cash_balance or data
    if let Some(cash) = data.get("cash_balance") {
        return Ok(format!("¥{} remaining", cash));
    }

    Err(format!("unexpected response format: {json}"))
}

async fn fetch_minimax(client: &Client, api_key: &str, group_id: &str) -> ApiBalance {
    let name = "Minimax".to_string();
    let result = do_fetch_minimax(client, api_key, group_id).await;
    ApiBalance { name, result }
}

async fn do_fetch_minimax(
    client: &Client,
    api_key: &str,
    group_id: &str,
) -> Result<String, String> {
    let url = format!(
        "https://api.minimax.chat/v1/query/token_usage?GroupId={group_id}"
    );

    let resp = client
        .get(&url)
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

    // Minimax returns balance info in various formats
    if let Some(balance) = json.get("total_balance").and_then(|v| v.as_f64()) {
        let used = json
            .get("total_used")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        return Ok(format!("¥{:.2} remaining (used ¥{:.2})", balance - used, used));
    }
    if let Some(balance) = json.get("balance").and_then(|v| v.as_f64()) {
        return Ok(format!("¥{:.2} remaining", balance));
    }

    // Fallback: show raw response
    Err(format!("unexpected response format: {json}"))
}
