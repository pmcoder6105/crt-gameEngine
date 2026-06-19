//! Mesh loading from .obj and .gltf/.glb.
//!
//! Both parsers produce a [`MeshData`]: parallel `positions`/`normals`/`uvs`
//! arrays (one entry per unique vertex) plus a triangle `indices` list. The
//! renderer side interleaves these into its `Vertex` layout on upload, keyed by
//! file path so re-loading the same asset is free (see the app's asset manager).

use std::collections::HashMap;
use std::path::Path;

use elderforge_core::math::{Vec2, Vec3};

use crate::SceneError;

/// CPU-side mesh data, ready to upload through the renderer's ResourceCache.
///
/// `positions`, `normals`, and `uvs` are parallel: vertex `i` is
/// `(positions[i], normals[i], uvs[i])`. `indices` are triangle indices into
/// those arrays (always a multiple of three).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Number of unique vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Recompute smooth per-vertex normals by accumulating the geometric normal
    /// of every triangle into its three vertices, then normalizing. Used when a
    /// source file carries no normals of its own.
    fn recompute_normals(&mut self) {
        self.normals = vec![Vec3::ZERO; self.positions.len()];
        for tri in self.indices.chunks_exact(3) {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let face = (self.positions[b] - self.positions[a])
                .cross(self.positions[c] - self.positions[a]);
            // `face` length is proportional to triangle area, so larger faces
            // weight the accumulated normal more — the usual area weighting.
            self.normals[a] += face;
            self.normals[b] += face;
            self.normals[c] += face;
        }
        for n in &mut self.normals {
            *n = n.normalize_or_zero();
        }
    }

    /// Pad `normals`/`uvs` out to `positions.len()` with defaults, so a source
    /// that omits one attribute (or supplies it for only some primitives) still
    /// yields aligned parallel arrays.
    fn pad_attributes(&mut self) {
        if self.normals.len() < self.positions.len() {
            self.normals.resize(self.positions.len(), Vec3::ZERO);
        }
        if self.uvs.len() < self.positions.len() {
            self.uvs.resize(self.positions.len(), Vec2::ZERO);
        }
    }
}

/// Load a mesh, picking the parser from the file extension.
pub fn load_mesh(path: &Path) -> Result<MeshData, SceneError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("obj") => load_obj(path),
        Some("gltf") | Some("glb") => load_gltf(path),
        other => Err(SceneError::UnsupportedFormat(
            other.unwrap_or("<none>").to_string(),
        )),
    }
}

/// Parse a Wavefront OBJ file.
///
/// Handles `v`/`vn`/`vt` and `f` lines with any of the `v`, `v/vt`, `v//vn`, and
/// `v/vt/vn` vertex forms, negative (relative) indices, and polygon faces (fan
/// triangulated). Each distinct `(position, uv, normal)` index triple becomes
/// one output vertex; missing normals are recomputed from the geometry.
fn load_obj(path: &Path) -> Result<MeshData, SceneError> {
    let text = std::fs::read_to_string(path)?;

    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();

    let mut out = MeshData::default();
    // Dedup (position, uv, normal) index triples into output vertices.
    let mut combined: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut any_normals = false;

    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        let mut tokens = line.split_whitespace();
        let parse_err = |what: &str| {
            SceneError::Parse(format!("OBJ line {}: malformed {what}", line_no + 1))
        };
        match tokens.next() {
            Some("v") => positions.push(parse_vec3(&mut tokens).ok_or_else(|| parse_err("v"))?),
            Some("vn") => normals.push(parse_vec3(&mut tokens).ok_or_else(|| parse_err("vn"))?),
            Some("vt") => {
                // `vt` may carry a third (w) coordinate; we use only u, v.
                let u = parse_f32(tokens.next()).ok_or_else(|| parse_err("vt"))?;
                let v = parse_f32(tokens.next()).unwrap_or(0.0);
                uvs.push(Vec2::new(u, v));
            }
            Some("f") => {
                let face: Vec<&str> = tokens.collect();
                if face.len() < 3 {
                    return Err(parse_err("f (needs >= 3 vertices)"));
                }
                let mut face_indices = Vec::with_capacity(face.len());
                for vert in &face {
                    let (pi, ti, ni) = parse_face_vertex(vert)
                        .ok_or_else(|| parse_err("f vertex"))?;
                    let pos = resolve_index(pi, positions.len()).ok_or_else(|| parse_err("f position index"))?;
                    let tex = match ti {
                        Some(ti) => Some(resolve_index(ti, uvs.len()).ok_or_else(|| parse_err("f uv index"))?),
                        None => None,
                    };
                    let nor = match ni {
                        Some(ni) => Some(resolve_index(ni, normals.len()).ok_or_else(|| parse_err("f normal index"))?),
                        None => None,
                    };
                    let key = (
                        pos as i32,
                        tex.map(|t| t as i32).unwrap_or(-1),
                        nor.map(|n| n as i32).unwrap_or(-1),
                    );
                    let index = *combined.entry(key).or_insert_with(|| {
                        out.positions.push(positions[pos]);
                        out.uvs.push(tex.map(|t| uvs[t]).unwrap_or(Vec2::ZERO));
                        if let Some(n) = nor {
                            out.normals.push(normals[n]);
                            any_normals = true;
                        } else {
                            out.normals.push(Vec3::ZERO);
                        }
                        (out.positions.len() - 1) as u32
                    });
                    face_indices.push(index);
                }
                // Fan triangulation: (0, k, k+1) for k in 1..n-1.
                for k in 1..face_indices.len() - 1 {
                    out.indices.push(face_indices[0]);
                    out.indices.push(face_indices[k]);
                    out.indices.push(face_indices[k + 1]);
                }
            }
            // Comments and unsupported directives (o, g, s, mtllib, usemtl, …).
            _ => {}
        }
    }

    if out.positions.is_empty() {
        return Err(SceneError::Parse(format!(
            "OBJ '{}' contained no geometry",
            path.display()
        )));
    }
    if !any_normals {
        out.recompute_normals();
    }
    out.pad_attributes();
    Ok(out)
}

/// Parse three whitespace-separated floats into a `Vec3`.
fn parse_vec3<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<Vec3> {
    let x = parse_f32(tokens.next())?;
    let y = parse_f32(tokens.next())?;
    let z = parse_f32(tokens.next())?;
    Some(Vec3::new(x, y, z))
}

fn parse_f32(token: Option<&str>) -> Option<f32> {
    token.and_then(|t| t.parse().ok())
}

/// Parse one `f` vertex token (`v`, `v/vt`, `v//vn`, or `v/vt/vn`) into 1-based
/// (or negative-relative) position / texcoord / normal indices.
fn parse_face_vertex(token: &str) -> Option<(i32, Option<i32>, Option<i32>)> {
    let mut parts = token.split('/');
    let pi = parts.next()?.parse::<i32>().ok()?;
    let ti = match parts.next() {
        Some("") | None => None,
        Some(s) => Some(s.parse::<i32>().ok()?),
    };
    let ni = match parts.next() {
        Some("") | None => None,
        Some(s) => Some(s.parse::<i32>().ok()?),
    };
    Some((pi, ti, ni))
}

/// Resolve a 1-based (positive) or relative (negative) OBJ index against the
/// current count, returning a 0-based index or `None` if out of range.
fn resolve_index(index: i32, count: usize) -> Option<usize> {
    let resolved = if index > 0 {
        (index - 1) as i64
    } else if index < 0 {
        count as i64 + index as i64
    } else {
        return None; // index 0 is invalid in OBJ
    };
    if resolved < 0 || resolved as usize >= count {
        None
    } else {
        Some(resolved as usize)
    }
}

/// Parse a glTF / GLB file, concatenating every primitive of every mesh into a
/// single [`MeshData`]. Per-primitive index runs are offset so they keep
/// referencing the right vertices after concatenation.
fn load_gltf(path: &Path) -> Result<MeshData, SceneError> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| SceneError::Parse(format!("glTF import: {e}")))?;

    let mut out = MeshData::default();
    let mut any_normals = false;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions = reader
                .read_positions()
                .ok_or_else(|| SceneError::Parse("glTF primitive without positions".into()))?;
            let base = out.positions.len() as u32;
            for p in positions {
                out.positions.push(Vec3::from_array(p));
            }
            let added = out.positions.len() as u32 - base;

            if let Some(normals) = reader.read_normals() {
                for n in normals {
                    out.normals.push(Vec3::from_array(n));
                }
                any_normals = true;
            }
            if let Some(uvs) = reader.read_tex_coords(0) {
                for uv in uvs.into_f32() {
                    out.uvs.push(Vec2::from_array(uv));
                }
            }
            // Keep the parallel arrays aligned even if this primitive omitted an
            // attribute the previous one supplied.
            out.pad_attributes();

            match reader.read_indices() {
                Some(indices) => {
                    for i in indices.into_u32() {
                        out.indices.push(base + i);
                    }
                }
                // Non-indexed primitive: vertices are consumed in order.
                None => out.indices.extend(base..base + added),
            }
        }
    }

    if out.positions.is_empty() {
        return Err(SceneError::Parse(format!(
            "glTF '{}' contained no geometry",
            path.display()
        )));
    }
    if !any_normals {
        out.recompute_normals();
    }
    out.pad_attributes();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write `contents` to a uniquely named temp file with `ext` and return it.
    fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn parses_obj_triangle_with_normals_and_uvs() {
        let obj = "\
v 0 0 0
v 1 0 0
v 0 1 0
vt 0 0
vt 1 0
vt 0 1
vn 0 0 1
f 1/1/1 2/2/1 3/3/1
";
        let path = temp_file("elderforge_tri.obj", obj);
        let mesh = load_mesh(&path).expect("load obj");
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        assert_eq!(mesh.positions[1], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(mesh.uvs[2], Vec2::new(0.0, 1.0));
        // Normal points along +Z as written.
        assert!((mesh.normals[0] - Vec3::Z).length() < 1e-6);
    }

    #[test]
    fn triangulates_quad_and_recomputes_missing_normals() {
        // A quad in the XY plane, no normals supplied (positions only).
        let obj = "\
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
f 1 2 3 4
";
        let path = temp_file("elderforge_quad.obj", obj);
        let mesh = load_mesh(&path).expect("load obj");
        assert_eq!(mesh.vertex_count(), 4);
        // A 4-gon fans into two triangles -> 6 indices.
        assert_eq!(mesh.indices.len(), 6);
        // Recomputed normals all face +Z (CCW winding in the XY plane).
        for n in &mesh.normals {
            assert!((n.z - 1.0).abs() < 1e-6, "expected +Z normal, got {n:?}");
        }
    }

    #[test]
    fn negative_indices_are_relative() {
        let obj = "\
v 0 0 0
v 1 0 0
v 0 1 0
f -3 -2 -1
";
        let path = temp_file("elderforge_neg.obj", obj);
        let mesh = load_mesh(&path).expect("load obj");
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn empty_obj_is_an_error() {
        let path = temp_file("elderforge_empty.obj", "# just a comment\n");
        assert!(matches!(load_mesh(&path), Err(SceneError::Parse(_))));
    }

    #[test]
    fn unknown_extension_is_unsupported() {
        let path = temp_file("elderforge_mesh.xyz", "");
        assert!(matches!(
            load_mesh(&path),
            Err(SceneError::UnsupportedFormat(_))
        ));
    }
}
