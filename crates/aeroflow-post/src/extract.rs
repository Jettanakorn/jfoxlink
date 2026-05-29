use std::path::Path;

pub struct FieldExtractor;

impl Default for FieldExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldExtractor {
    pub fn new() -> Self {
        Self
    }

    pub async fn extract_yplus(&self, case_path: &str) -> Result<(f64, f64), anyhow::Error> {
        let path = Path::new(case_path).join("postProcessing/yPlus/0/yPlus.dat");
        let data = std::fs::read_to_string(&path)?;

        let mut max_y = 0.0f64;
        let mut sum_y = 0.0f64;
        let mut count = 0u64;

        for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(v) = parts[1].parse::<f64>() {
                    max_y = max_y.max(v);
                    sum_y += v;
                    count += 1;
                }
        }

        let mean_y = if count > 0 { sum_y / count as f64 } else { 0.0 };
        Ok((max_y, mean_y))
    }

    pub async fn extract_cp(&self, case_path: &str, patch: &str) -> Result<Vec<(f64, f64, f64)>, anyhow::Error> {
        let _ = (case_path, patch);
        Ok(vec![])
    }

    pub async fn extract_probes(&self, case_path: &str) -> Result<Vec<ProbeData>, anyhow::Error> {
        let _ = case_path;
        Ok(vec![])
    }
}

pub struct ProbeData {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub pressure: f64,
}
