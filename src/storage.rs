use std::{path::PathBuf, task::Poll};

use eframe::egui::ImageSource;
use futures::FutureExt;

pub struct Storage {
    base: PathBuf,
}

impl Storage {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }
    // uri -> Display
    pub fn list_images(
        &self,
    ) -> futures::future::BoxFuture<'static, std::io::Result<Vec<(String, String)>>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let path = self.base.to_path_buf();
        std::thread::spawn(|| {
            let r = Self::list_images_blocking(path);
            tx.send(r)
        });
        async move {
            rx.await
                .map_err(|e| std::io::Error::other(e))
                .and_then(|a| a)
        }
        .boxed()
    }

    fn list_images_blocking(path: PathBuf) -> std::io::Result<Vec<(String, String)>> {
        let files = std::fs::read_dir(path)?;
        Ok(files
            .into_iter()
            .map(|x| {
                let x = x?;
                Ok((
                    x.path().to_string_lossy().to_string(),
                    x.file_name().to_string_lossy().to_string(),
                ))
            })
            .collect::<std::io::Result<Vec<_>>>()?)
    }
}
