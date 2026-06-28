use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

fn get_default_rates() -> HashMap<String, f64> {
    let mut rates = HashMap::new();
    rates.insert("USD".to_string(), 1.0);
    rates.insert("EUR".to_string(), 0.92);
    rates.insert("GBP".to_string(), 0.79);
    rates.insert("JPY".to_string(), 155.0);
    rates.insert("CAD".to_string(), 1.36);
    rates.insert("AUD".to_string(), 1.50);
    rates.insert("BRL".to_string(), 5.40);
    rates.insert("CNY".to_string(), 7.25);
    rates
}

fn load_rates(cache_path: Option<&str>) -> HashMap<String, f64> {
    let mut paths = Vec::new();
    if let Some(p) = cache_path {
        paths.push(PathBuf::from(p));
    } else {
        paths.push(PathBuf::from("currencies.json"));
        if let Ok(home_dir) = std::env::var("HOME") {
            let mut p = PathBuf::from(home_dir);
            p.push(".config");
            p.push("numen");
            p.push("currencies.json");
            paths.push(p);
        }
    }

    for path in paths {
        if path.exists() {
            if let Ok(mut file) = File::open(&path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                        // Check for standard "rates" key in the exchange API response
                        if let Some(rates_obj) = parsed.get("rates").or_else(|| parsed.get("conversion_rates")) {
                            if let Ok(rates) = serde_json::from_value::<HashMap<String, f64>>(rates_obj.clone()) {
                                return rates;
                            }
                        }
                        // Check if it's a flat Map of { "USD": 1.0, ... }
                        if let Ok(rates) = serde_json::from_value::<HashMap<String, f64>>(parsed) {
                            return rates;
                        }
                    }
                }
            }
        }
    }

    get_default_rates()
}

pub fn convert_currency(val: f64, from: &str, to: &str, cache_path: Option<&str>) -> Result<f64, String> {
    let from_upper = from.to_uppercase();
    let to_upper = to.to_uppercase();
    if from_upper == to_upper {
        return Ok(val);
    }

    let rates = load_rates(cache_path);

    let from_rate = rates.get(&from_upper)
        .ok_or_else(|| format!("Unsupported currency: '{}'", from_upper))?;
    let to_rate = rates.get(&to_upper)
        .ok_or_else(|| format!("Unsupported currency: '{}'", to_upper))?;

    // API rates are USD-centric (e.g. USD = 1.0, EUR = 0.92).
    // Convert to USD (val / from_rate) then convert to target (val_usd * to_rate)
    let val_usd = val / from_rate;
    let val_target = val_usd * to_rate;
    Ok(val_target)
}
