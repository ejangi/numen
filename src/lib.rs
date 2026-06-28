use pyo3::prelude::*;

mod parser;
mod units;
mod currency;
mod eval;

#[pyfunction]
#[pyo3(signature = (lines, cache_path=None))]
fn evaluate(lines: Vec<String>, cache_path: Option<String>) -> PyResult<Vec<String>> {
    let cache_ref = cache_path.as_deref();
    let results = eval::evaluate_document(&lines, cache_ref);
    Ok(results)
}

#[pymodule]
fn numen_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_eval() {
        let val = eval::evaluate_document(&["20 mins + 45 mins".to_string()], None);
        assert_eq!(val, vec!["65 mins".to_string()]);
    }

    #[test]
    fn test_prose_and_parentheses() {
        let lines = vec![
            "# Numen — Natural Language Notepad Calculator".to_string(),
            "# 4. Active Currencies (Multi-Currency Support)".to_string(),
            "Total distance in miles: 3.5 miles + 2 km in miles".to_string(),
        ];
        let val = eval::evaluate_document(&lines, None);
        assert_eq!(val[0], "");
        assert_eq!(val[1], "");
        assert_eq!(val[2], "4.7427 miles");
    }

    #[test]
    fn test_variables() {
        let lines = vec![
            "NoahYears2Go = 8".to_string(),
            "LaraYears2Go = 11".to_string(),
            "KidsYears2Go = NoahYears2Go + LaraYears2Go".to_string(),
        ];
        let val = eval::evaluate_document(&lines, None);
        assert_eq!(val[0], "8");
        assert_eq!(val[1], "11");
        assert_eq!(val[2], "19");
    }
}
