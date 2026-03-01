pub mod analytics_repo;
pub mod conversation_repo;
pub mod document_repo;
pub mod embedding_repo;
pub mod mcp_repo;
pub mod memory_repo;
pub mod model_repo;
pub mod settings_repo;
pub mod system_repo;
pub mod user_repo;

pub fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{blob_to_vector, vector_to_blob};

    #[test]
    fn vector_blob_roundtrip() {
        let source = vec![0.25f32, -0.5, 1.25, 3.75];
        let blob = vector_to_blob(&source);
        let restored = blob_to_vector(&blob);
        assert_eq!(source, restored);
    }
}
