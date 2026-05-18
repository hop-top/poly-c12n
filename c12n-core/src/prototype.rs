use thiserror::Error;

use crate::embedding::cosine_similarity;

#[derive(Debug, Error)]
pub enum PrototypeError {
    #[error("prototype bank is empty")]
    Empty,
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("invalid weight: {0}")]
    InvalidWeight(String),
}

pub struct PrototypeBank {
    prototypes: Vec<Vec<f32>>,
    weights: Vec<f32>,
    best_weight: f32,
    top_m: usize,
    dimension: usize,
}

impl PrototypeBank {
    pub fn new(
        prototypes: Vec<Vec<f32>>,
        weights: Vec<f32>,
        best_weight: f32,
        top_m: usize,
    ) -> Result<Self, PrototypeError> {
        if prototypes.is_empty() {
            return Err(PrototypeError::Empty);
        }
        if weights.len() != prototypes.len() {
            return Err(PrototypeError::InvalidWeight(format!(
                "weights length {} != prototypes length {}",
                weights.len(),
                prototypes.len()
            )));
        }
        if !(0.0..=1.0).contains(&best_weight) {
            return Err(PrototypeError::InvalidWeight(format!(
                "best_weight {best_weight} not in [0.0, 1.0]"
            )));
        }

        let dimension = prototypes[0].len();
        if dimension == 0 {
            return Err(PrototypeError::Empty);
        }

        for (i, p) in prototypes.iter().enumerate() {
            if p.len() != dimension {
                return Err(PrototypeError::DimensionMismatch {
                    expected: dimension,
                    got: p.len(),
                });
            }
            if !weights[i].is_finite() || weights[i] < 0.0 {
                return Err(PrototypeError::InvalidWeight(format!(
                    "weight[{i}] = {} is invalid",
                    weights[i]
                )));
            }
        }

        Ok(Self {
            prototypes,
            weights,
            best_weight,
            top_m: top_m.max(1),
            dimension,
        })
    }

    pub fn score(&self, query: &[f32]) -> Result<f64, PrototypeError> {
        if query.len() != self.dimension {
            return Err(PrototypeError::DimensionMismatch {
                expected: self.dimension,
                got: query.len(),
            });
        }

        let mut weighted_sims: Vec<f32> = self
            .prototypes
            .iter()
            .zip(self.weights.iter())
            .map(|(proto, &w)| cosine_similarity(proto, query) * w)
            .collect();

        weighted_sims.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let max_sim = weighted_sims[0] as f64;

        let m = self.top_m.min(weighted_sims.len());
        let mean_top_m: f64 = weighted_sims[..m].iter().map(|&s| s as f64).sum::<f64>() / m as f64;

        let bw = self.best_weight as f64;
        Ok(bw * max_sim + (1.0 - bw) * mean_top_m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_bank(vecs: Vec<Vec<f32>>, best_weight: f32, top_m: usize) -> PrototypeBank {
        let n = vecs.len();
        PrototypeBank::new(vecs, vec![1.0; n], best_weight, top_m).unwrap()
    }

    #[test]
    fn score_identical_query() {
        let bank = uniform_bank(vec![vec![1.0, 0.0, 0.0, 0.0]], 0.5, 1);
        let score = bank.score(&[1.0, 0.0, 0.0, 0.0]).unwrap();
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn score_orthogonal_query() {
        let bank = uniform_bank(vec![vec![1.0, 0.0, 0.0, 0.0]], 0.5, 1);
        let score = bank.score(&[0.0, 1.0, 0.0, 0.0]).unwrap();
        assert!(score.abs() < 1e-6);
    }

    #[test]
    fn score_blends_best_and_top_m() {
        let bank = uniform_bank(
            vec![
                vec![1.0, 0.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0, 0.0],
            ],
            0.6,
            2,
        );
        // query aligned with first prototype
        let score = bank.score(&[1.0, 0.0, 0.0, 0.0]).unwrap();
        // max_sim = 1.0, mean_top_2 = (1.0 + 0.0) / 2 = 0.5
        // score = 0.6 * 1.0 + 0.4 * 0.5 = 0.8
        assert!((score - 0.8).abs() < 1e-6);
    }

    #[test]
    fn dimension_mismatch_rejected() {
        let bank = uniform_bank(vec![vec![1.0, 0.0, 0.0, 0.0]], 0.5, 1);
        assert!(bank.score(&[1.0, 0.0]).is_err());
    }

    #[test]
    fn empty_prototypes_rejected() {
        let r = PrototypeBank::new(vec![], vec![], 0.5, 1);
        assert!(r.is_err());
    }

    #[test]
    fn invalid_best_weight_rejected() {
        let r = PrototypeBank::new(vec![vec![1.0, 0.0]], vec![1.0], 1.5, 1);
        assert!(r.is_err());
    }

    #[test]
    fn negative_prototype_weight_rejected() {
        let r = PrototypeBank::new(vec![vec![1.0, 0.0]], vec![-1.0], 0.5, 1);
        assert!(r.is_err());
    }
}
