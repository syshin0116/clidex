//! Semantic search using Model2Vec embeddings (feature-gated behind "semantic")

use std::io::{Read, Write};
use std::path::Path;

/// Embedding dimensions (potion-base-2M uses 64-dim)
pub const EMBED_DIM: usize = 64;

/// Load embeddings from binary file
/// Format: [u32 count][u32 dim][f32 * count * dim]
pub fn load_embeddings(path: &Path) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 4];

    file.read_exact(&mut buf)?;
    let count = u32::from_le_bytes(buf) as usize;
    file.read_exact(&mut buf)?;
    let dim = u32::from_le_bytes(buf) as usize;

    let mut embeddings = Vec::with_capacity(count);
    for _ in 0..count {
        let mut vec = vec![0f32; dim];
        let bytes = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr() as *mut u8, dim * 4) };
        file.read_exact(bytes)?;
        embeddings.push(vec);
    }

    Ok(embeddings)
}

/// Save embeddings to binary file
pub fn save_embeddings(
    embeddings: &[Vec<f32>],
    dim: usize,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = std::fs::File::create(path)?;
    let count = embeddings.len() as u32;

    file.write_all(&count.to_le_bytes())?;
    file.write_all(&(dim as u32).to_le_bytes())?;

    for emb in embeddings {
        let bytes = unsafe { std::slice::from_raw_parts(emb.as_ptr() as *const u8, dim * 4) };
        file.write_all(bytes)?;
    }

    Ok(())
}

/// Cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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
