use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("failed to load model: {0}")]
    ModelLoad(String),
    #[error("inference error: {0}")]
    Inference(String),
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("batch too large: max {max}, got {got}")]
    BatchTooLarge { max: usize, got: usize },
}

#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimension(&self) -> usize;
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vector lengths must match");

    let len = a.len();
    let chunks = len / 4;

    let mut dot = [0.0f32; 4];
    let mut norm_a = [0.0f32; 4];
    let mut norm_b = [0.0f32; 4];

    for i in 0..chunks {
        let base = i * 4;
        for j in 0..4 {
            let ai = a[base + j];
            let bi = b[base + j];
            dot[j] += ai * bi;
            norm_a[j] += ai * ai;
            norm_b[j] += bi * bi;
        }
    }

    let mut dot_sum = dot[0] + dot[1] + dot[2] + dot[3];
    let mut norm_a_sum = norm_a[0] + norm_a[1] + norm_a[2] + norm_a[3];
    let mut norm_b_sum = norm_b[0] + norm_b[1] + norm_b[2] + norm_b[3];

    for i in (chunks * 4)..len {
        let ai = a[i];
        let bi = b[i];
        dot_sum += ai * bi;
        norm_a_sum += ai * ai;
        norm_b_sum += bi * bi;
    }

    let denom = (norm_a_sum * norm_b_sum).sqrt();
    if denom == 0.0 {
        return 0.0;
    }

    dot_sum / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn opposite_vectors() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![-1.0, -2.0, -3.0, -4.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_vector() {
        let a = vec![0.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn non_multiple_of_four() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let b = vec![7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let expected = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
            / (a.iter().map(|x| x * x).sum::<f32>().sqrt()
                * b.iter().map(|x| x * x).sum::<f32>().sqrt());
        let sim = cosine_similarity(&a, &b);
        assert!((sim - expected).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "vector lengths must match")]
    fn mismatched_lengths() {
        cosine_similarity(&[1.0, 2.0], &[1.0]);
    }
}
