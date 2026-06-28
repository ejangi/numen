#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Length,
    Time,
    Mass,
}

#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub canonical_name: &'static str,
    pub dimension: Dimension,
    pub factor: f64, // Multiply value by this factor to convert to base unit
}

// Base units: Length -> "meter", Time -> "second", Mass -> "gram"
pub fn lookup_unit(name: &str) -> Option<UnitInfo> {
    let norm = name.trim().to_lowercase();
    match norm.as_str() {
        // Length
        "m" | "meter" | "meters" => Some(UnitInfo {
            canonical_name: "meter",
            dimension: Dimension::Length,
            factor: 1.0,
        }),
        "km" | "kilometer" | "kilometers" => Some(UnitInfo {
            canonical_name: "meter",
            dimension: Dimension::Length,
            factor: 1000.0,
        }),
        "mile" | "miles" => Some(UnitInfo {
            canonical_name: "meter",
            dimension: Dimension::Length,
            factor: 1609.344,
        }),

        // Time
        "s" | "sec" | "secs" | "second" | "seconds" => Some(UnitInfo {
            canonical_name: "second",
            dimension: Dimension::Time,
            factor: 1.0,
        }),
        "min" | "mins" | "minute" | "minutes" => Some(UnitInfo {
            canonical_name: "second",
            dimension: Dimension::Time,
            factor: 60.0,
        }),
        "h" | "hour" | "hours" => Some(UnitInfo {
            canonical_name: "second",
            dimension: Dimension::Time,
            factor: 3600.0,
        }),

        // Mass
        "g" | "gram" | "grams" => Some(UnitInfo {
            canonical_name: "gram",
            dimension: Dimension::Mass,
            factor: 1.0,
        }),
        "kg" | "kilogram" | "kilograms" => Some(UnitInfo {
            canonical_name: "gram",
            dimension: Dimension::Mass,
            factor: 1000.0,
        }),
        "lbs" | "pound" | "pounds" => Some(UnitInfo {
            canonical_name: "gram",
            dimension: Dimension::Mass,
            factor: 453.59237,
        }),

        _ => None,
    }
}

pub fn convert_unit(val: f64, from_unit: &str, to_unit: &str) -> Result<f64, String> {
    let from_info = lookup_unit(from_unit)
        .ok_or_else(|| format!("Unknown unit: '{}'", from_unit))?;
    let to_info = lookup_unit(to_unit)
        .ok_or_else(|| format!("Unknown unit: '{}'", to_unit))?;

    if from_info.dimension != to_info.dimension {
        return Err(format!(
            "Cannot convert from '{}' to '{}' (mismatched dimensions)",
            from_unit, to_unit
        ));
    }

    let val_base = val * from_info.factor;
    let val_target = val_base / to_info.factor;
    Ok(val_target)
}
