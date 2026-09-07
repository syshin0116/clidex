//! Semantic search using Model2Vec embeddings.

use crate::model::Tool;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

type Error = Box<dyn std::error::Error>;
pub const MODEL_ID: &str = "minishlab/potion-base-2M";
pub const EMBED_DIM: usize = 64;
const MAX_EMBEDDINGS_COUNT: usize = 1_000_000;
const MAX_EMBED_DIM: usize = 4096;
const MAGIC: &[u8; 8] = b"CLDXEMB1";

pub fn embedding_text(tool: &Tool) -> String {
    format!("{} {} {}", tool.name, tool.desc, tool.tags.join(" "))
}

fn metadata(tools: &[Tool]) -> Result<Vec<u8>, Error> {
    // Exact model inputs bind vectors to content and order without a hash dependency.
    Ok(serde_json::to_vec(&(
        MODEL_ID,
        tools.iter().map(embedding_text).collect::<Vec<_>>(),
    ))?)
}

fn read_vectors(mut reader: impl Read, file_len: u64) -> Result<Vec<Vec<f32>>, Error> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    let count = u32::from_le_bytes(bytes) as usize;
    reader.read_exact(&mut bytes)?;
    let dim = u32::from_le_bytes(bytes) as usize;
    if count > MAX_EMBEDDINGS_COUNT || dim == 0 || dim > MAX_EMBED_DIM {
        return Err("Invalid embedding dimensions".into());
    }
    if file_len != 8 + count as u64 * dim as u64 * 4 {
        return Err("Embedding file size does not match its header".into());
    }
    let mut embeddings = Vec::with_capacity(count);
    for _ in 0..count {
        let mut vector = Vec::with_capacity(dim);
        for _ in 0..dim {
            reader.read_exact(&mut bytes)?;
            let value = f32::from_le_bytes(bytes);
            if !value.is_finite() {
                return Err("Non-finite embedding value".into());
            }
            vector.push(value);
        }
        embeddings.push(vector);
    }
    Ok(embeddings)
}

fn validate_vectors(embeddings: &[Vec<f32>], dim: usize) -> Result<(), Error> {
    if dim == 0
        || dim > MAX_EMBED_DIM
        || embeddings.len() > MAX_EMBEDDINGS_COUNT
        || embeddings
            .iter()
            .any(|vector| vector.len() != dim || vector.iter().any(|v| !v.is_finite()))
    {
        return Err("Invalid embedding dimensions or values".into());
    }
    Ok(())
}

fn write_vectors(mut writer: impl Write, embeddings: &[Vec<f32>], dim: usize) -> Result<(), Error> {
    writer.write_all(&(embeddings.len() as u32).to_le_bytes())?;
    writer.write_all(&(dim as u32).to_le_bytes())?;
    for vector in embeddings {
        for value in vector {
            writer.write_all(&value.to_le_bytes())?;
        }
    }
    writer.flush()?;
    Ok(())
}

/// Read the original count/dimension/vector format.
pub fn load_embeddings(path: &Path) -> Result<Vec<Vec<f32>>, Error> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    read_vectors(BufReader::new(file), len)
}

pub fn save_embeddings(embeddings: &[Vec<f32>], dim: usize, path: &Path) -> Result<(), Error> {
    validate_vectors(embeddings, dim)?;
    write_vectors(
        BufWriter::new(std::fs::File::create(path)?),
        embeddings,
        dim,
    )
}

pub fn load_tool_embeddings(path: &Path, tools: &[Tool]) -> Result<Vec<Vec<f32>>, Error> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    read_tool_embeddings(BufReader::new(file), len, tools)
}

pub(crate) fn read_tool_embeddings(
    mut reader: impl Read,
    len: u64,
    tools: &[Tool],
) -> Result<Vec<Vec<f32>>, Error> {
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err("Embedding format changed; run clidex update".into());
    }
    let expected = metadata(tools)?;
    let mut size = [0; 4];
    reader.read_exact(&mut size)?;
    if u32::from_le_bytes(size) as usize != expected.len() {
        return Err("Embeddings belong to another index".into());
    }
    let mut stored = vec![0; expected.len()];
    reader.read_exact(&mut stored)?;
    if stored != expected {
        return Err("Embedding model or index content mismatch".into());
    }
    let remaining = len
        .checked_sub(12 + stored.len() as u64)
        .ok_or("Truncated embeddings")?;
    let vectors = read_vectors(reader, remaining)?;
    if vectors.len() != tools.len() || vectors.iter().any(|vector| vector.len() != EMBED_DIM) {
        return Err("Embedding shape does not match model and index".into());
    }
    Ok(vectors)
}

pub fn save_tool_embeddings(
    embeddings: &[Vec<f32>],
    tools: &[Tool],
    path: &Path,
) -> Result<(), Error> {
    validate_vectors(embeddings, EMBED_DIM)?;
    if embeddings.len() != tools.len() {
        return Err("Embedding count does not match tools".into());
    }
    let metadata = metadata(tools)?;
    let size = u32::try_from(metadata.len())?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(MAGIC)?;
    writer.write_all(&size.to_le_bytes())?;
    writer.write_all(&metadata)?;
    write_vectors(writer, embeddings, EMBED_DIM)
}

/// Cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Reciprocal Rank Fusion to combine two ranked lists
/// Returns combined scores indexed by tool position
pub fn rrf_combine(
    bm25_ranked: &[(usize, f64)], // (tool_idx, score) sorted by score desc
    semantic_ranked: &[(usize, f32)], // (tool_idx, similarity) sorted by sim desc
    total_tools: usize,
    k: f64,
) -> Vec<(usize, f64)> {
    let mut rrf_scores = vec![0.0f64; total_tools];

    for (rank, (idx, _)) in bm25_ranked.iter().enumerate() {
        rrf_scores[*idx] += 1.0 / (k + rank as f64 + 1.0);
    }

    for (rank, (idx, _)) in semantic_ranked.iter().enumerate() {
        rrf_scores[*idx] += 1.0 / (k + rank as f64 + 1.0);
    }

    let mut combined: Vec<(usize, f64)> = rrf_scores
        .into_iter()
        .enumerate()
        .filter(|(_, s)| *s > 0.0)
        .collect();
    combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    combined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_files_validate_shape_identity_and_preserve_previous_data() {
        let path =
            std::env::temp_dir().join(format!("clidex-embeddings-{}.bin", std::process::id()));
        let tool: Tool = serde_json::from_value(
            serde_json::json!({"name":"jq","desc":"JSON processor","category":"Data"}),
        )
        .unwrap();
        let mut other = tool.clone();
        other.name = "fx".into();
        let tools = vec![tool, other];
        let vectors = vec![vec![0.25; EMBED_DIM], vec![0.5; EMBED_DIM]];
        save_tool_embeddings(&vectors, &tools, &path).unwrap();
        assert_eq!(load_tool_embeddings(&path, &tools).unwrap(), vectors);
        let mut changed = tools.clone();
        changed[0].desc = "YAML processor".into();
        assert!(load_tool_embeddings(&path, &changed).is_err());
        let mut reordered = tools.clone();
        reordered.reverse();
        assert!(load_tool_embeddings(&path, &reordered).is_err());
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_tool_embeddings(&[vec![1.0]], &tools, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(save_embeddings(&[vec![f32::NAN]], 1, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
        assert!(load_tool_embeddings(&path, &tools).is_err());
        save_embeddings(&[vec![1.5, -2.0]], 2, &path).unwrap();
        assert_eq!(
            &std::fs::read(&path).unwrap()[8..12],
            &1.5_f32.to_le_bytes()
        );
        assert_eq!(load_embeddings(&path).unwrap(), vec![vec![1.5, -2.0]]);
        assert!(load_tool_embeddings(&path, &tools).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
