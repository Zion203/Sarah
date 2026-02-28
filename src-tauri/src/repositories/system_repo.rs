use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::SystemProfile;
use crate::error::AppError;

#[derive(Clone)]
pub struct SystemRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl SystemRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            read_pool: pool.clone(),
            write_pool: pool,
        }
    }

    pub fn with_pools(read_pool: SqlitePool, write_pool: SqlitePool) -> Self {
        Self {
            read_pool,
            write_pool,
        }
    }

    pub async fn upsert_profile(
        &self,
        mut profile: SystemProfile,
    ) -> Result<SystemProfile, AppError> {
        if profile.id.trim().is_empty() {
            profile.id = Uuid::new_v4().to_string();
        }

        sqlx::query(
            r#"
            INSERT INTO system_profile (
              id, cpu_brand, cpu_cores, cpu_threads, cpu_frequency_mhz, total_ram_mb,
              available_ram_mb, gpu_name, gpu_vendor, gpu_vram_mb, gpu_backend,
              storage_total_gb, storage_available_gb, storage_type, os_name, os_version,
              os_arch, platform, benchmark_tokens_per_sec, benchmark_embed_ms,
              capability_score, supports_cuda, supports_metal, supports_vulkan,
              supports_avx2, supports_avx512, last_scan_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)
            ON CONFLICT(id) DO UPDATE SET
              cpu_brand = excluded.cpu_brand,
              cpu_cores = excluded.cpu_cores,
              cpu_threads = excluded.cpu_threads,
              cpu_frequency_mhz = excluded.cpu_frequency_mhz,
              total_ram_mb = excluded.total_ram_mb,
              available_ram_mb = excluded.available_ram_mb,
              gpu_name = excluded.gpu_name,
              gpu_vendor = excluded.gpu_vendor,
              gpu_vram_mb = excluded.gpu_vram_mb,
              gpu_backend = excluded.gpu_backend,
              storage_total_gb = excluded.storage_total_gb,
              storage_available_gb = excluded.storage_available_gb,
              storage_type = excluded.storage_type,
              os_name = excluded.os_name,
              os_version = excluded.os_version,
              os_arch = excluded.os_arch,
              platform = excluded.platform,
              benchmark_tokens_per_sec = excluded.benchmark_tokens_per_sec,
              benchmark_embed_ms = excluded.benchmark_embed_ms,
              capability_score = excluded.capability_score,
              supports_cuda = excluded.supports_cuda,
              supports_metal = excluded.supports_metal,
              supports_vulkan = excluded.supports_vulkan,
              supports_avx2 = excluded.supports_avx2,
              supports_avx512 = excluded.supports_avx512,
              last_scan_at = excluded.last_scan_at
            "#,
        )
        .bind(&profile.id)
        .bind(&profile.cpu_brand)
        .bind(profile.cpu_cores)
        .bind(profile.cpu_threads)
        .bind(profile.cpu_frequency_mhz)
        .bind(profile.total_ram_mb)
        .bind(profile.available_ram_mb)
        .bind(&profile.gpu_name)
        .bind(&profile.gpu_vendor)
        .bind(profile.gpu_vram_mb)
        .bind(&profile.gpu_backend)
        .bind(profile.storage_total_gb)
        .bind(profile.storage_available_gb)
        .bind(&profile.storage_type)
        .bind(&profile.os_name)
        .bind(&profile.os_version)
        .bind(&profile.os_arch)
        .bind(&profile.platform)
        .bind(profile.benchmark_tokens_per_sec)
        .bind(profile.benchmark_embed_ms)
        .bind(profile.capability_score)
        .bind(profile.supports_cuda)
        .bind(profile.supports_metal)
        .bind(profile.supports_vulkan)
        .bind(profile.supports_avx2)
        .bind(profile.supports_avx512)
        .bind(&profile.last_scan_at)
        .execute(&self.write_pool)
        .await?;

        self.get_profile_by_id(&profile.id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "system_profile".to_string(),
                id: profile.id,
            })
    }

    pub async fn get_current_profile(&self) -> Result<Option<SystemProfile>, AppError> {
        let row = sqlx::query_as::<_, SystemProfile>(
            "SELECT * FROM system_profile ORDER BY last_scan_at DESC LIMIT 1",
        )
        .fetch_optional(&self.read_pool)
        .await?;
        Ok(row)
    }

    pub async fn get_profile_by_id(&self, id: &str) -> Result<Option<SystemProfile>, AppError> {
        let row = sqlx::query_as::<_, SystemProfile>("SELECT * FROM system_profile WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn update_benchmark(
        &self,
        id: &str,
        tokens_per_sec: f64,
        embed_ms: f64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE system_profile SET benchmark_tokens_per_sec = ?1, benchmark_embed_ms = ?2 WHERE id = ?3",
        )
        .bind(tokens_per_sec)
        .bind(embed_ms)
        .bind(id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn update_capability_score(&self, id: &str, score: f64) -> Result<(), AppError> {
        sqlx::query("UPDATE system_profile SET capability_score = ?1 WHERE id = ?2")
            .bind(score)
            .bind(id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }
}
