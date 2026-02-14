use crate::config::{Token, WalletEntry};
use reqwest::Client;
use serde_json::{json, Value};

/// USDC contract on Base mainnet
const BASE_USDC_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

pub struct WalletBalance {
    pub name: String,
    pub address: String,
    pub token: Token,
    pub balance: Result<f64, String>,
}

pub async fn fetch_wallet_balances(
    client: &Client,
    rpc_url: &str,
    wallets: &[WalletEntry],
) -> Vec<WalletBalance> {
    let futs: Vec<_> = wallets
        .iter()
        .map(|w| fetch_one(client, rpc_url, w))
        .collect();
    futures::future::join_all(futs).await
}

async fn fetch_one(client: &Client, rpc_url: &str, wallet: &WalletEntry) -> WalletBalance {
    let result = match wallet.token {
        Token::ETH => fetch_eth_balance(client, rpc_url, &wallet.address).await,
        Token::USDC => fetch_erc20_balance(client, rpc_url, BASE_USDC_ADDRESS, &wallet.address, wallet.token.decimals()).await,
    };
    WalletBalance {
        name: wallet.name.clone(),
        address: wallet.address.clone(),
        token: wallet.token,
        balance: result,
    }
}

async fn fetch_eth_balance(client: &Client, rpc_url: &str, address: &str) -> Result<f64, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [address, "latest"],
        "id": 1
    });

    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {e}"))?;

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse RPC response: {e}"))?;

    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {error}"));
    }

    let hex_str = json["result"]
        .as_str()
        .ok_or_else(|| "Missing result in RPC response".to_string())?;

    parse_hex_balance(hex_str, 18)
}

async fn fetch_erc20_balance(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    holder: &str,
    decimals: u32,
) -> Result<f64, String> {
    // balanceOf(address) selector = 0x70a08231, then address padded to 32 bytes
    let addr_clean = holder.strip_prefix("0x").unwrap_or(holder);
    let data = format!("0x70a08231{:0>64}", addr_clean);

    let body = json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract, "data": data}, "latest"],
        "id": 1
    });

    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {e}"))?;

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse RPC response: {e}"))?;

    if let Some(error) = json.get("error") {
        return Err(format!("RPC error: {error}"));
    }

    let hex_str = json["result"]
        .as_str()
        .ok_or_else(|| "Missing result in RPC response".to_string())?;

    parse_hex_balance(hex_str, decimals)
}

fn parse_hex_balance(hex_str: &str, decimals: u32) -> Result<f64, String> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let raw = u128::from_str_radix(hex_str, 16)
        .map_err(|e| format!("Failed to parse hex balance: {e}"))?;
    Ok(raw as f64 / 10_f64.powi(decimals as i32))
}
