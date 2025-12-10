use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Deserialize;

const EXCHANGE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";

#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
	symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Deserialize)]
struct SymbolInfo {
	symbol: String,
	filters: Vec<Filter>,
}

#[derive(Debug, Deserialize)]
struct Filter {
	#[serde(rename = "filterType")]
	filter_type: String,
	#[serde(rename = "tickSize")]
	tick_size: Option<String>,
	#[serde(rename = "stepSize")]
	step_size: Option<String>,
}

pub async fn fetch_tick_and_step_sizes(symbol: &str) -> Result<(Decimal, Decimal)> {
	let url = format!("{}?symbol={}", EXCHANGE_INFO_URL, symbol.to_uppercase());
	let response = reqwest::get(&url).await?;
	let json_value: serde_json::Value = response.json().await?;

	if json_value.get("code").is_some() {
		let msg = json_value
			.get("msg")
			.and_then(|m| m.as_str())
			.unwrap_or("Unknown error");
		anyhow::bail!("Binance API error for symbol \"{}\": {}", symbol, msg);
	}

	let info: ExchangeInfoResponse = serde_json::from_value(json_value)
		.context("Failed to parse exchange info response")?;

	let symbol_info = info
		.symbols
		.into_iter()
		.find(|s| s.symbol.eq_ignore_ascii_case(symbol))
		.with_context(|| format!("Symbol \"{}\" not found in exchange info", symbol))?;

	let tick_size_str = symbol_info
		.filters
		.iter()
		.find(|f| f.filter_type == "PRICE_FILTER")
		.and_then(|f| f.tick_size.as_ref())
		.with_context(|| format!("Tick size not found for symbol \"{}\"", symbol))?;

	let step_size_str = symbol_info
		.filters
		.iter()
		.find(|f| f.filter_type == "LOT_SIZE")
		.and_then(|f| f.step_size.as_ref())
		.with_context(|| format!("Step size not found for symbol \"{}\"", symbol))?;

	let tick_size = tick_size_str
		.parse::<Decimal>()
		.with_context(|| format!("Failed to parse tick size \"{}\"", tick_size_str))?;

	let step_size = step_size_str
		.parse::<Decimal>()
		.with_context(|| format!("Failed to parse step size \"{}\"", step_size_str))?;

	Ok((tick_size, step_size))
}
