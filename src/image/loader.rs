//! Image loading and caching.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use image::DynamicImage;

#[derive(Debug, Default)]
struct CacheInner {
    entries: HashMap<PathBuf, DynamicImage>,
    order: VecDeque<PathBuf>,
}

/// Cache for loaded images.
#[derive(Debug, Default)]
pub struct ImageCache {
    inner: Arc<Mutex<CacheInner>>,
    max_size: usize,
}

impl ImageCache {
    /// Create a new image cache with the given maximum number of entries.
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CacheInner::default())),
            max_size,
        }
    }

    /// Get an image from the cache.
    pub fn get(&self, path: &Path) -> Option<DynamicImage> {
        let guard = self.inner.lock().ok()?;
        guard.entries.get(path).cloned()
    }

    /// Insert an image into the cache.
    pub fn insert(&self, path: PathBuf, image: DynamicImage) {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if guard.entries.contains_key(&path) {
            guard.entries.insert(path, image);
            return;
        }

        guard.order.push_back(path.clone());
        guard.entries.insert(path.clone(), image);

        while guard.entries.len() > self.max_size {
            if let Some(oldest) = guard.order.pop_front() {
                guard.entries.remove(&oldest);
            } else {
                break;
            }
        }
    }

    /// Check if an image is in the cache.
    pub fn contains(&self, path: &Path) -> bool {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.entries.contains_key(path)
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.entries.clear();
        guard.order.clear();
    }

    /// Get the number of cached images.
    pub fn len(&self) -> usize {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Image loader with caching.
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
    pub fn load(&self, image_path: &str) -> Option<DynamicImage> {
        let full_path = self.resolve_path(image_path);

        if let Some(img) = self.cache.get(&full_path) {
            return Some(img);
        }

        let img = image::open(&full_path).ok()?;
        self.cache.insert(full_path, img.clone());
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
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_new() {
        let cache = ImageCache::new(10);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_len() {
        let cache = ImageCache::new(10);
        assert_eq!(cache.len(), 0);
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
