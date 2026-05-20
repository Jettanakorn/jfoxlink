pub struct PostReader;

impl PostReader {
    pub fn new() -> Self {
        Self
    }

    pub async fn read_vtu(&self, path: &str) -> Result<MeshData, anyhow::Error> {
        // P3: use vtkio crate to read unstructured grid from .vtu file
        let _ = path;
        Ok(MeshData {
            n_points: 0,
            n_cells: 0,
            point_data: std::collections::HashMap::new(),
            cell_data: std::collections::HashMap::new(),
        })
    }
}

pub struct MeshData {
    pub n_points: usize,
    pub n_cells: usize,
    pub point_data: std::collections::HashMap<String, Vec<f64>>,
    pub cell_data: std::collections::HashMap<String, Vec<f64>>,
}
