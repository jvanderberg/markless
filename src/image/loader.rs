//! Async image loading and caching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::DynamicImage;
use tokio::sync::RwLock;

/// Cache for loaded images.
#[derive(Debug, Default)]
pub struct ImageCache {
    cache: Arc<RwLock<HashMap<PathBuf, DynamicImage>>>,
    max_size: usize,
}

impl ImageCache {
    /// Create a new image cache with the given maximum number of entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    /// Get an image from the cache.
    pub async fn get(&self, path: &Path) -> Option<DynamicImage> {
        let cache = self.cache.read().await;
        cache.get(path).cloned()
    }

    /// Insert an image into the cache.
    pub async fn insert(&self, path: PathBuf, image: DynamicImage) {
        let mut cache = self.cache.write().await;

        // If at capacity, remove oldest entry (simple FIFO for now)
        if cache.len() >= self.max_size {
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }

        cache.insert(path, image);
    }

    /// Check if an image is in the cache.
    pub async fn contains(&self, path: &Path) -> bool {
        let cache = self.cache.read().await;
        cache.contains_key(path)
    }

    /// Clear the cache.
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get the number of cached images.
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Check if the cache is empty.
    pub async fn is_empty(&self) -> bool {
        let cache = self.cache.read().await;
        cache.is_empty()
    }
}

/// Async image loader with caching.
pub struct ImageLoader {
    cache: ImageCache,
    base_path: PathBuf,
}

impl ImageLoader {
    /// Create a new image loader with the given base path for relative images.
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            cache: ImageCache::new(50), // Cache up to 50 images
            base_path,
        }
    }

    /// Load an image, using cache if available.
    pub async fn load(&self, image_path: &str) -> Option<DynamicImage> {
        let full_path = self.resolve_path(image_path);

        // Check cache first
        if let Some(img) = self.cache.get(&full_path).await {
            return Some(img);
        }

        // Load from disk
        let img = image::open(&full_path).ok()?;

        // Cache it
        self.cache.insert(full_path, img.clone()).await;

        Some(img)
    }

    /// Load an image synchronously (for use in non-async contexts).
    pub fn load_sync(&self, image_path: &str) -> Option<DynamicImage> {
        let full_path = self.resolve_path(image_path);
        image::open(&full_path).ok()
    }

    /// Resolve a potentially relative path to an absolute path.
    fn resolve_path(&self, image_path: &str) -> PathBuf {
        let path = Path::new(image_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        }
    }

    /// Get the base path.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Clear the image cache.
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_new() {
        let cache = ImageCache::new(10);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_cache_len() {
        let cache = ImageCache::new(10);
        assert_eq!(cache.len().await, 0);
    }

    #[test]
    fn test_loader_resolve_path_absolute() {
        let loader = ImageLoader::new(PathBuf::from("/base"));
        let resolved = loader.resolve_path("/absolute/path.png");
        assert_eq!(resolved, PathBuf::from("/absolute/path.png"));
    }

    #[test]
    fn test_loader_resolve_path_relative() {
        let loader = ImageLoader::new(PathBuf::from("/base"));
        let resolved = loader.resolve_path("relative/path.png");
        assert_eq!(resolved, PathBuf::from("/base/relative/path.png"));
    }
}
