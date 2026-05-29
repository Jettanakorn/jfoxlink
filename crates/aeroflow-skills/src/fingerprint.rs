use aeroflow_core::GeometryId;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::Path;
use uuid::Uuid;

// ── Voxel signature types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxelSignature {
    pub coarse_hash: Vec<u8>,   // 8³ = 512 bits → 64 bytes (bit-packed)
    pub medium_hash: Vec<u8>,   // 32³ = 32K bits → 4 KB (bit-packed)
    pub fine_hash: Vec<u8>,     // SHA-256 of 64³ grid
    pub voxel_data: Option<Vec<u8>>,  // 64³ = 262144 bytes (stored optionally)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryFingerprint {
    pub geometry_id: GeometryId,
    pub sha256_hash: Vec<u8>,
    pub voxel_signature: VoxelSignature,
    pub bbox: [f64; 6],
    pub surface_area: f64,
    pub volume: f64,
    pub aspect_ratio: f64,
    pub num_triangles: u32,
}

// ── Triangle helpers ──

/// Möller–Trumbore ray-triangle intersection.
/// Returns `Some(t)` if ray from `origin` in direction `dir` hits the triangle,
/// where `t` is the distance along the ray.
fn ray_triangle_intersect(
    origin: [f64; 3],
    dir: [f64; 3],
    v0: [f64; 3],
    v1: [f64; 3],
    v2: [f64; 3],
) -> Option<f64> {
    let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

    let h = [
        dir[1] * edge2[2] - dir[2] * edge2[1],
        dir[2] * edge2[0] - dir[0] * edge2[2],
        dir[0] * edge2[1] - dir[1] * edge2[0],
    ];

    let a = edge1[0] * h[0] + edge1[1] * h[1] + edge1[2] * h[2];
    if a.abs() < 1e-15 {
        return None;
    }

    let f = 1.0 / a;
    let s = [origin[0] - v0[0], origin[1] - v0[1], origin[2] - v0[2]];
    let u = f * (s[0] * h[0] + s[1] * h[1] + s[2] * h[2]);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = [
        s[1] * edge1[2] - s[2] * edge1[1],
        s[2] * edge1[0] - s[0] * edge1[2],
        s[0] * edge1[1] - s[1] * edge1[0],
    ];

    let v = f * (dir[0] * q[0] + dir[1] * q[1] + dir[2] * q[2]);
    if !(0.0..=1.0).contains(&v) {
        return None;
    }

    let t = f * (edge2[0] * q[0] + edge2[1] * q[1] + edge2[2] * q[2]);
    if t > 1e-10 { Some(t) } else { None }
}

/// Test if a point is inside a closed triangle mesh using ray casting.
///
/// Utility available for future use in alternative voxelization strategies.
#[allow(dead_code)]
fn point_inside_mesh(point: [f64; 3], tris: &[[f64; 3]; 3], vertices: &[[f64; 3]]) -> bool {
    let ray_dir = [1.0, 0.0, 0.0];
    let mut intersections = 0;
    for i in 0..tris.len() {
        let v0 = vertices[tris[i][0] as usize];
        let v1 = vertices[tris[i][1] as usize];
        let v2 = vertices[tris[i][2] as usize];
        if ray_triangle_intersect(point, ray_dir, v0, v1, v2).is_some() {
            intersections += 1;
        }
    }
    intersections % 2 == 1
}

/// Bit-pack a flat bool array into bytes (MSB first).
fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut bytes = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            bytes[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    bytes
}

// ── Implementation ──

impl GeometryFingerprint {
    /// Read an STL file and compute full geometry fingerprint including voxelization.
    pub fn from_stl(path: &Path) -> Result<Self, anyhow::Error> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);

        // SHA-256 of raw file
        let sha256_hash = {
            let mut file = std::fs::File::open(path)?;
            let mut hasher = sha2::Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            hasher.finalize().to_vec()
        };

        let mesh = stl_io::read_stl(&mut reader)?;
        let num_triangles = mesh.faces.len() as u32;

        // Convert vertices to f64
        let vertices: Vec<[f64; 3]> = mesh.vertices.iter()
            .map(|v| [v[0] as f64, v[1] as f64, v[2] as f64])
            .collect();

        // Convert faces to index triples
        let tris: Vec<[usize; 3]> = mesh.faces.iter()
            .map(|f| [f.vertices[0], f.vertices[1], f.vertices[2]])
            .collect();

        // Bounding box
        let mut bbox = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
        for v in &vertices {
            if v[0] < bbox[0] { bbox[0] = v[0]; }
            if v[1] < bbox[1] { bbox[1] = v[1]; }
            if v[2] < bbox[2] { bbox[2] = v[2]; }
            if v[0] > bbox[3] { bbox[3] = v[0]; }
            if v[1] > bbox[4] { bbox[4] = v[1]; }
            if v[2] > bbox[5] { bbox[5] = v[2]; }
        }

        let extents = [bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]];
        let surface_area = if extents[0] > 0.0 && extents[1] > 0.0 && extents[2] > 0.0 {
            2.0 * (extents[0]*extents[1] + extents[0]*extents[2] + extents[1]*extents[2])
        } else {
            0.0
        };
        let volume = extents[0] * extents[1] * extents[2];
        let aspect_ratio = if extents[0] > 0.0 {
            extents.iter().cloned().fold(f64::MIN, f64::max)
                / extents.iter().cloned().fold(f64::MAX, f64::min)
        } else {
            1.0
        };

        // ── Voxelization at 64³ ──
        let resolution = 64;
        let mut voxels_64 = vec![false; resolution * resolution * resolution];

        // Pad the bounding box by 10% to ensure surface voxels are captured
        let pad_x = extents[0] * 0.05;
        let pad_y = extents[1] * 0.05;
        let pad_z = extents[2] * 0.05;
        let x_min = bbox[0] - pad_x;
        let y_min = bbox[1] - pad_y;
        let z_min = bbox[2] - pad_z;
        let x_max = bbox[3] + pad_x;
        let y_max = bbox[4] + pad_y;
        let z_max = bbox[5] + pad_z;
        let dx = (x_max - x_min) / resolution as f64;
        let dy = (y_max - y_min) / resolution as f64;
        let dz = (z_max - z_min) / resolution as f64;

        // Pre-extract triangle arrays for fast access
        let tri_arrays: Vec<[[f64; 3]; 3]> = tris.iter()
            .map(|&[i0, i1, i2]| [vertices[i0], vertices[i1], vertices[i2]])
            .collect();

        // Ray-cast each voxel center
        let _tris_slice: &[[usize; 3]] = &tris;
        for iz in 0..resolution {
            let cz = z_min + (iz as f64 + 0.5) * dz;
            for iy in 0..resolution {
                let cy = y_min + (iy as f64 + 0.5) * dy;
                for ix in 0..resolution {
                    let cx = x_min + (ix as f64 + 0.5) * dx;
                    let _point = [cx, cy, cz];

                    // Fast AABB pre-check: skip voxels clearly outside
                    if cx < bbox[0] || cx > bbox[3] || cy < bbox[1] || cy > bbox[4] || cz < bbox[2] || cz > bbox[5] {
                        continue;
                    }

                    // Ray cast in +x direction
                    let mut hits = 0u32;
                    let _ray_origin = [cx, cy, cz];
                    // Avoid self-intersection by nudging slightly
                    let nudge = 1e-8;
                    let ray_origin_nudged = [cx + nudge, cy, cz];

                    for tri in &tri_arrays {
                        if ray_triangle_intersect(ray_origin_nudged, [1.0, 0.0, 0.0], tri[0], tri[1], tri[2]).is_some() {
                            hits += 1;
                        }
                    }

                    if hits % 2 == 1 {
                        let idx = iz * resolution * resolution + iy * resolution + ix;
                        voxels_64[idx] = true;
                    }
                }
            }
        }

        // Downsample to 32³ (average 2×2×2 blocks)
        let res_32 = 32;
        let mut voxels_32 = vec![false; res_32 * res_32 * res_32];
        let scale_32 = resolution / res_32;
        for iz in 0..res_32 {
            for iy in 0..res_32 {
                for ix in 0..res_32 {
                    let mut sum = 0u32;
                    for dz_i in 0..scale_32 {
                        for dy_i in 0..scale_32 {
                            for dx_i in 0..scale_32 {
                                let idx_64 = (iz * scale_32 + dz_i) * resolution * resolution
                                    + (iy * scale_32 + dy_i) * resolution
                                    + (ix * scale_32 + dx_i);
                                if voxels_64[idx_64] {
                                    sum += 1;
                                }
                            }
                        }
                    }
                    let threshold = (scale_32 * scale_32 * scale_32) as u32 / 2;
                    let idx_32 = iz * res_32 * res_32 + iy * res_32 + ix;
                    voxels_32[idx_32] = sum >= threshold;
                }
            }
        }

        // Downsample to 8³ (average 4×4×4 blocks from 32³)
        let res_8 = 8;
        let mut voxels_8 = vec![false; res_8 * res_8 * res_8];
        let scale_8 = res_32 / res_8;
        for iz in 0..res_8 {
            for iy in 0..res_8 {
                for ix in 0..res_8 {
                    let mut sum = 0u32;
                    for dz_i in 0..scale_8 {
                        for dy_i in 0..scale_8 {
                            for dx_i in 0..scale_8 {
                                let idx_32 = (iz * scale_8 + dz_i) * res_32 * res_32
                                    + (iy * scale_8 + dy_i) * res_32
                                    + (ix * scale_8 + dx_i);
                                if voxels_32[idx_32] {
                                    sum += 1;
                                }
                            }
                        }
                    }
                    let threshold = (scale_8 * scale_8 * scale_8) as u32 / 2;
                    let idx_8 = iz * res_8 * res_8 + iy * res_8 + ix;
                    voxels_8[idx_8] = sum >= threshold;
                }
            }
        }

        // Hash the 64³ grid with SHA-256
        let voxel_bytes: Vec<u8> = voxels_64.iter().map(|&b| b as u8).collect();
        let fine_hash = sha2::Sha256::digest(&voxel_bytes).to_vec();

        // Bit-pack coarse and medium hashes
        let coarse_hash = pack_bits(&voxels_8);
        let medium_hash = pack_bits(&voxels_32);

        // Optionally store the full 64³ voxel data
        let voxel_data = Some(pack_bits(&voxels_64));

        Ok(Self {
            geometry_id: Uuid::new_v4(),
            sha256_hash,
            voxel_signature: VoxelSignature {
                coarse_hash,
                medium_hash,
                fine_hash,
                voxel_data,
            },
            bbox,
            surface_area,
            volume,
            aspect_ratio,
            num_triangles,
        })
    }

    /// Read an STL file and compute a quick fingerprint (SHA-256 + bbox, no voxelization).
    /// Use for upload flows where speed matters; full voxelization can use from_stl().
    pub fn from_stl_quick(path: &Path) -> Result<Self, anyhow::Error> {
        // SHA-256 of raw file
        let sha256_hash = {
            let mut file = std::fs::File::open(path)?;
            let mut hasher = sha2::Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            hasher.finalize().to_vec()
        };

        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mesh = stl_io::read_stl(&mut reader)?;
        let num_triangles = mesh.faces.len() as u32;

        let vertices: Vec<[f64; 3]> = mesh.vertices.iter()
            .map(|v| [v[0] as f64, v[1] as f64, v[2] as f64])
            .collect();

        let _tris: Vec<[usize; 3]> = mesh.faces.iter()
            .map(|f| [f.vertices[0], f.vertices[1], f.vertices[2]])
            .collect();

        // Bounding box
        let mut bbox = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
        for v in &vertices {
            if v[0] < bbox[0] { bbox[0] = v[0]; }
            if v[1] < bbox[1] { bbox[1] = v[1]; }
            if v[2] < bbox[2] { bbox[2] = v[2]; }
            if v[0] > bbox[3] { bbox[3] = v[0]; }
            if v[1] > bbox[4] { bbox[4] = v[1]; }
            if v[2] > bbox[5] { bbox[5] = v[2]; }
        }

        let extents = [bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]];
        let volume = extents[0] * extents[1] * extents[2];
        let surface_area = if volume > 0.0 {
            2.0 * (extents[0]*extents[1] + extents[0]*extents[2] + extents[1]*extents[2])
        } else {
            0.0
        };
        let aspect_ratio = if extents[0] > 0.0 {
            extents.iter().cloned().fold(f64::MIN, f64::max)
                / extents.iter().cloned().fold(f64::MAX, f64::min)
        } else {
            1.0
        };

        Ok(GeometryFingerprint {
            geometry_id: Uuid::new_v4(),
            sha256_hash,
            voxel_signature: VoxelSignature {
                coarse_hash: vec![0u8; 64],
                medium_hash: vec![0u8; 4096],
                fine_hash: vec![0u8; 32],
                voxel_data: None,
            },
            bbox,
            surface_area,
            volume,
            aspect_ratio,
            num_triangles,
        })
    }

    /// Compute fingerprint from already-loaded vertex/triangle data.
    pub fn compute_fingerprint(vertices: &[[f64; 3]], triangles: &[[usize; 3]]) -> GeometryFingerprint {
        let mut bbox = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
        for v in vertices {
            if v[0] < bbox[0] { bbox[0] = v[0]; }
            if v[1] < bbox[1] { bbox[1] = v[1]; }
            if v[2] < bbox[2] { bbox[2] = v[2]; }
            if v[0] > bbox[3] { bbox[3] = v[0]; }
            if v[1] > bbox[4] { bbox[4] = v[1]; }
            if v[2] > bbox[5] { bbox[5] = v[2]; }
        }

        let extents = [bbox[3] - bbox[0], bbox[4] - bbox[1], bbox[5] - bbox[2]];
        let volume = extents[0] * extents[1] * extents[2];
        let surface_area = if volume > 0.0 {
            2.0 * (extents[0]*extents[1] + extents[0]*extents[2] + extents[1]*extents[2])
        } else {
            0.0
        };
        let aspect_ratio = if extents[0] > 0.0 {
            extents.iter().cloned().fold(f64::MIN, f64::max)
                / extents.iter().cloned().fold(f64::MAX, f64::min)
        } else {
            1.0
        };

        GeometryFingerprint {
            geometry_id: Uuid::new_v4(),
            sha256_hash: vec![0u8; 32],
            voxel_signature: VoxelSignature {
                coarse_hash: vec![0u8; 64],
                medium_hash: vec![0u8; 4096],
                fine_hash: vec![0u8; 32],
                voxel_data: None,
            },
            bbox,
            surface_area,
            volume,
            aspect_ratio,
            num_triangles: triangles.len() as u32,
        }
    }

    /// Hamming distance between two voxel signatures (coarse + medium weighted).
    pub fn hamming_distance(&self, other: &VoxelSignature) -> f64 {
        let coarse_dist: u32 = self.voxel_signature.coarse_hash
            .iter()
            .zip(&other.coarse_hash)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        let medium_dist: u64 = self.voxel_signature.medium_hash
            .iter()
            .zip(&other.medium_hash)
            .map(|(a, b)| (a ^ b).count_ones() as u64)
            .sum();

        // Normalize: coarse is 8³=512 bits, medium is 32³=32768 bits
        let coarse_norm = coarse_dist as f64 / 512.0;
        let medium_norm = medium_dist as f64 / 32768.0;

        // Weighted combination (coarse = 40%, medium = 60%)
        0.4 * coarse_norm + 0.6 * medium_norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_hamming_distance_self_is_zero() {
        let fp = GeometryFingerprint {
            geometry_id: Uuid::new_v4(),
            sha256_hash: vec![0u8; 32],
            voxel_signature: VoxelSignature {
                coarse_hash: vec![0u8; 64],
                medium_hash: vec![0u8; 4096],
                fine_hash: vec![0u8; 32],
                voxel_data: None,
            },
            bbox: [0.0; 6],
            surface_area: 0.0,
            volume: 0.0,
            aspect_ratio: 1.0,
            num_triangles: 0,
        };
        let dist = fp.hamming_distance(&fp.voxel_signature);
        assert_eq!(dist, 0.0);
    }

    #[test]
    fn test_hamming_distance_different_is_positive() {
        let fp = GeometryFingerprint {
            geometry_id: Uuid::new_v4(),
            sha256_hash: vec![0u8; 32],
            voxel_signature: VoxelSignature {
                coarse_hash: vec![0u8; 64],
                medium_hash: vec![0u8; 4096],
                fine_hash: vec![0u8; 32],
                voxel_data: None,
            },
            bbox: [0.0; 6],
            surface_area: 0.0,
            volume: 0.0,
            aspect_ratio: 1.0,
            num_triangles: 0,
        };
        let other_vs = VoxelSignature {
            coarse_hash: vec![0xFFu8; 64],
            medium_hash: vec![0xFFu8; 4096],
            fine_hash: vec![0u8; 32],
            voxel_data: None,
        };
        let dist = fp.hamming_distance(&other_vs);
        assert!((dist - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_fingerprint() {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 1, 2], [0, 2, 3],
            [4, 6, 5], [4, 7, 6],
            [0, 4, 5], [0, 5, 1],
            [3, 2, 6], [3, 6, 7],
            [0, 3, 7], [0, 7, 4],
            [1, 5, 6], [1, 6, 2],
        ];
        let fp = GeometryFingerprint::compute_fingerprint(&vertices, &triangles);
        assert_eq!(fp.num_triangles, 12);
        assert_eq!(fp.bbox, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert!((fp.volume - 1.0).abs() < 1e-10);
        assert!((fp.surface_area - 6.0).abs() < 1e-10);
        assert!((fp.aspect_ratio - 1.0).abs() < 1e-10);
        assert_eq!(fp.voxel_signature.coarse_hash.len(), 64);
        assert_eq!(fp.voxel_signature.medium_hash.len(), 4096);
        assert_eq!(fp.voxel_signature.fine_hash.len(), 32);
    }

    #[test]
    fn test_pack_bits() {
        let bits = vec![true, false, true, false, true, false, true, false];
        let packed = pack_bits(&bits);
        assert_eq!(packed, vec![0b10101010]);

        let bits_short = vec![true, true, false];
        let packed_short = pack_bits(&bits_short);
        assert_eq!(packed_short, vec![0b11000000]);

        let bits_long = vec![true; 9];
        let packed_long = pack_bits(&bits_long);
        assert_eq!(packed_long, vec![0xFF, 0x80]);
    }

    #[test]
    fn test_ray_triangle_intersect_hit() {
        let v0 = [0.0, 0.0, 0.0];
        let v1 = [1.0, 0.0, 0.0];
        let v2 = [0.0, 1.0, 0.0];
        let origin = [0.1, 0.1, -1.0];
        let dir = [0.0, 0.0, 1.0];
        let t = ray_triangle_intersect(origin, dir, v0, v1, v2);
        assert!(t.is_some());
        assert!((t.unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ray_triangle_intersect_miss() {
        let v0 = [0.0, 0.0, 0.0];
        let v1 = [1.0, 0.0, 0.0];
        let v2 = [0.0, 1.0, 0.0];
        let origin = [0.1, 0.1, -1.0];
        let dir = [0.0, 0.0, -1.0];
        let t = ray_triangle_intersect(origin, dir, v0, v1, v2);
        assert!(t.is_none());
    }

    #[test]
    fn test_ray_triangle_intersect_parallel() {
        let v0 = [0.0, 0.0, 0.0];
        let v1 = [1.0, 0.0, 0.0];
        let v2 = [0.0, 1.0, 0.0];
        let origin = [0.1, 0.1, 1.0];
        let dir = [1.0, 0.0, 0.0];
        let t = ray_triangle_intersect(origin, dir, v0, v1, v2);
        assert!(t.is_none());
    }
}
