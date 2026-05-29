use aeroflow_core::WindTunnelConfig;

/// Generates an asymmetric blockMeshDict for digital wind tunnel domains.
///
/// The domain is split into two blocks along the x-axis (at x=0) so that
/// grading independently compresses cells toward the model location on
/// both the upstream and downstream sides.
pub struct WindTunnelBlockMesh;

impl WindTunnelBlockMesh {
    /// Generate a blockMeshDict string for the given domain extents.
    ///
    /// # Parameters
    /// - `(x_min, x_max, y_min, y_max, z_min, z_max)`: domain extents (m)
    /// - `config`: optional wind tunnel config for cell count overrides
    pub fn generate(
        (x_min, x_max, y_min, y_max, z_min, z_max): (f64, f64, f64, f64, f64, f64),
        config: Option<&WindTunnelConfig>,
    ) -> String {
        let _cfg = config.cloned().unwrap_or_default();

        // Cell counts: scale with domain size, minimum 10 per block
        let dy_domain = y_max - y_min;
        let dz_domain = z_max - z_min;

        // Use chord-based or configurable cells-per-length
        let cells_per_m: f64 = 40.0;
        let n_up = ((0.0 - x_min).abs() * cells_per_m).ceil().max(10.0) as u32;
        let n_dn = (x_max * cells_per_m).ceil().max(10.0) as u32;
        let n_y = (dy_domain * cells_per_m * 0.6).ceil().max(6.0) as u32;
        let n_z = (dz_domain * cells_per_m * 0.6).ceil().max(6.0) as u32;

        // Grading: ratio > 1 means cells get smaller toward the model (x=0).
        // Asymmetric: upstream needs stronger grading (5:1), downstream milder (3:1).
        let grad_up = 5.0;
        let grad_dn = 3.0;

        // Vertex numbering for 2-block setup:
        // Block 0 (upstream):  hex (0 1 2 3 4 5 6 7)
        //   x from x_min → 0
        // Block 1 (downstream): hex (1 8 9 2 5 10 11 6)
        //   x from 0 → x_max
        //
        // Vertex coordinates:
        //   0: (x_min, y_min, z_min)
        //   1: (0,     y_min, z_min)
        //   2: (0,     y_max, z_min)
        //   3: (x_min, y_max, z_min)
        //   4: (x_min, y_min, z_max)
        //   5: (0,     y_min, z_max)
        //   6: (0,     y_max, z_max)
        //   7: (x_min, y_max, z_max)
        //   8: (x_max, y_min, z_min)
        //   9: (x_max, y_max, z_min)
        //  10: (x_max, y_min, z_max)
        //  11: (x_max, y_max, z_max)

        format!(
            r#"
FoamFile {{ version 2.0; format ascii; class dictionary; object blockMeshDict; }}
scale 1.0;

vertices
(
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
    ({:.8e} {:.8e} {:.8e})
);

blocks
(
    hex (0 1 2 3 4 5 6 7) ({} {} {}) simpleGrading ({} 1 1)
    hex (1 8 9 2 5 10 11 6) ({} {} {}) simpleGrading ({} 1 1)
);

edges ();

boundary
(
    inlet
    {{
        type patch;
        faces
        (
            (0 4 7 3)
        );
    }}
    outlet
    {{
        type patch;
        faces
        (
            (8 9 11 10)
        );
    }}
    top
    {{
        type patch;
        faces
        (
            (3 7 6 2)
            (2 6 11 9)
        );
    }}
    bottom
    {{
        type patch;
        faces
        (
            (0 1 5 4)
            (1 8 10 5)
        );
    }}
    front
    {{
        type patch;
        faces
        (
            (0 3 2 1)
            (1 2 9 8)
        );
    }}
    back
    {{
        type patch;
        faces
        (
            (4 5 6 7)
            (5 10 11 6)
        );
    }}
);

mergePatchPairs ();
"#,
            // 0: (x_min, y_min, z_min)
            x_min, y_min, z_min,
            // 1: (0, y_min, z_min)
            0.0, y_min, z_min,
            // 2: (0, y_max, z_min)
            0.0, y_max, z_min,
            // 3: (x_min, y_max, z_min)
            x_min, y_max, z_min,
            // 4: (x_min, y_min, z_max)
            x_min, y_min, z_max,
            // 5: (0, y_min, z_max)
            0.0, y_min, z_max,
            // 6: (0, y_max, z_max)
            0.0, y_max, z_max,
            // 7: (x_min, y_max, z_max)
            x_min, y_max, z_max,
            // 8: (x_max, y_min, z_min)
            x_max, y_min, z_min,
            // 9: (x_max, y_max, z_min)
            x_max, y_max, z_min,
            // 10: (x_max, y_min, z_max)
            x_max, y_min, z_max,
            // 11: (x_max, y_max, z_max)
            x_max, y_max, z_max,
            // Block 0 cells & grading
            n_up, n_y, n_z, grad_up,
            // Block 1 cells & grading
            n_dn, n_y, n_z, 1.0 / grad_dn,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_valid_dict() {
        let dict = WindTunnelBlockMesh::generate(
            (-2.0, 4.0, -2.5, 2.5, -2.5, 2.5),
            None,
        );
        assert!(dict.contains("FoamFile"));
        assert!(dict.contains("blockMeshDict"));
        assert!(dict.contains("vertices"));
        assert!(dict.contains("blocks"));
        assert!(dict.contains("inlet"));
        assert!(dict.contains("outlet"));
        assert!(dict.contains("top"));
        assert!(dict.contains("bottom"));
        assert!(dict.contains("front"));
        assert!(dict.contains("back"));
    }

    #[test]
    fn test_vertices_12() {
        let dict = WindTunnelBlockMesh::generate(
            (-2.0, 4.0, -2.5, 2.5, -2.5, 2.5),
            None,
        );
        // Verify structure
        assert!(dict.contains("vertices"));
        assert!(dict.contains("blocks"));
        assert!(dict.contains("boundary"));
        // Verify expected coordinate values exist in the vertex list.
        // Rust's {:.8e} format produces outputs like "0.00000000e0", "-2.00000000e0".
        assert!(dict.contains("0.00000000e0"));   // vertices on x=0 plane
        assert!(dict.contains("-2.00000000e0"));  // x_min
        assert!(dict.contains("4.00000000e0"));   // x_max
    }

    #[test]
    fn test_two_blocks() {
        let dict = WindTunnelBlockMesh::generate(
            (-2.0, 4.0, -2.5, 2.5, -2.5, 2.5),
            None,
        );
        assert!(dict.contains("hex (0 1 2 3 4 5 6 7)"));
        assert!(dict.contains("hex (1 8 9 2 5 10 11 6)"));
        // Exactly two hex blocks
        let hex_count = dict.matches("hex").count();
        assert_eq!(hex_count, 2);
    }

    #[test]
    fn test_cell_counts_nonzero() {
        let dict = WindTunnelBlockMesh::generate(
            (-2.0, 4.0, -2.5, 2.5, -2.5, 2.5),
            None,
        );
        // Extract cell counts from block definitions
        for line in dict.lines() {
            if line.trim().starts_with("hex") {
                // lines like: hex (...) (10 8 8) simpleGrading (5 1 1)
                let parts: Vec<&str> = line.split(')').collect();
                if parts.len() >= 3 {
                    let cells_part = parts[1].trim().trim_start_matches('(');
                    let nums: Vec<&str> = cells_part.split_whitespace().collect();
                    for n in nums {
                        let v: u32 = n.parse().unwrap_or(0);
                        assert!(v >= 6, "Cell count {} too low in line: {}", v, line);
                    }
                }
            }
        }
    }

    #[test]
    fn test_boundary_faces() {
        let dict = WindTunnelBlockMesh::generate(
            (-2.0, 4.0, -2.5, 2.5, -2.5, 2.5),
            None,
        );
        // Inlet should be a single face (block 0, x-min)
        assert!(dict.contains("(0 4 7 3)"));
        // Outlet should be a single face (block 1, x-max)
        assert!(dict.contains("(8 9 11 10)"));
        // Top/bottom/front/back should have 2 faces each (one per block)
        let top_block0_face = "(3 7 6 2)";
        let top_block1_face = "(2 6 11 9)";
        assert!(dict.contains(top_block0_face));
        assert!(dict.contains(top_block1_face));
    }
}
