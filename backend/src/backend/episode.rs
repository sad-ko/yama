use tracing::info;

use super::{video_metadata::VideoMetadata, Backend};
use crate::Meta;

use core::fmt::Debug;
use std::{fs, io, path::PathBuf};

#[derive(Debug)]
pub struct Episode {
    pub name: String,
    pub path: PathBuf,
    pub number: usize,
    pub thumbnail_path: PathBuf,
    pub md_path: PathBuf,
    pub metadata: VideoMetadata,
}

impl Episode {
    pub fn new(path: &PathBuf, number: usize) -> io::Result<Episode> {
        let md_folder = format!(".metadata/episode_{number}");
        let dir = unsafe { path.parent().unwrap_unchecked() };
        fs::create_dir_all(format!("{}/{}", dir.display(), &md_folder))?;

        let mut old_md_path = path.clone();
        old_md_path.set_extension("md");

        let md_path = unsafe {
            dir.join(&md_folder)
                .join(old_md_path.file_name().unwrap_unchecked())
        };

        let thumbnail_path = dir.join(&md_folder).join("thumbnail.jpg");

        if fs::metadata(&md_path).is_err() {
            // Non-performant way of generating metadata
            /*let cmd = if cfg!(target_os = "windows") {
                format!("--no-video,--end=0.1,{}", path.display())
            } else {
                format!("--no-video --end=0.1 \"{}\"", path.display())
            };

            Backend::run_mpv(&cmd)?;*/

            // More performant way
            /*let cmd = if cfg!(target_os = "windows") {
                format!("ffprobe,-v,error,-show_entries,format=duration,-of,default=noprint_wrappers=1:nokey=1,{}",
                path.display())
            } else {
                format!("ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 \"{}\"",
                path.display())
            };

            let output = Backend::run_process(&cmd)?;
            let duration: f64 = String::from_utf8(output.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap();*/

            // God help me
            let duration: f64 = ffprobe::ffprobe(path)
                .map_err(|_| {
                    fs::remove_dir(format!("{}/{}", dir.display(), &md_folder)).unwrap();
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{:?} is an invalid video", path.file_name().unwrap()),
                    )
                })?
                .format
                .get_duration()
                .ok_or_else(|| {
                    fs::remove_dir(format!("{}/{}", dir.display(), &md_folder)).unwrap();
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{:?} is an invalid video", path.file_name().unwrap()),
                    )
                })?
                .as_secs_f64();

            VideoMetadata::default_file(duration, &md_path)?;
        }

        // Optimize this: Lazy initialization of thumbnails
        if fs::metadata(&thumbnail_path).is_err() {
            let cmd = if cfg!(target_os = "windows") {
                format!(
                    "ffmpeg,-i,{},-vf,thumbnail,-frames:v,1,{},-f,mjpeg,-hide_banner,-nostdin,-nostats,-loglevel,quiet",
                    path.display(),
                    thumbnail_path.display(),
                )
            } else {
                format!(
                    "ffmpeg -i \"{}\" -vf \"thumbnail\" -frames:v 1 \"{}\" -f mjpeg -hide_banner -nostdin -nostats -loglevel quiet",
                    path.display(),
                    thumbnail_path.display(),
                )
            };

            Backend::run_process(&cmd)?;
        }

        Ok(Episode {
            name: unsafe {
                path.file_name()
                    .unwrap_unchecked()
                    .to_str()
                    .unwrap_unchecked()
                    .to_string()
            },
            metadata: VideoMetadata::new(&md_path)?,
            md_path,
            number,
            path: path.to_owned(),
            thumbnail_path,
        })
    }

    pub fn update_metadata(&mut self) -> io::Result<()> {
        let mut new_md_path = self.path.clone();
        new_md_path.set_extension("md");

        fs::rename(new_md_path, &self.md_path)?;
        self.metadata = VideoMetadata::new(&self.md_path)?;
        Ok(())
    }

    pub fn as_watched(&mut self) -> io::Result<()> {
        info!(
            "Mark {} as {}",
            self.name,
            if self.metadata.watched {
                "unwatched"
            } else {
                "watched"
            }
        );

        self.metadata.watched = !self.metadata.watched;
        self.metadata.current = if !self.metadata.watched {
            0.0
        } else {
            self.metadata.duration
        };
        VideoMetadata::create_file(&self.metadata, &self.md_path)
    }

    /// Runs episode on the current time (Restart it if it has been already watched)
    ///
    /// Returns [`io::Error`] if the mpv fails to launch it or the metadata can not be updated.
    pub fn run(&mut self) -> io::Result<()> {
        info!("Running {}", self.name);

        let cmd = if cfg!(target_os = "windows") {
            format!(
                "--start={},{}",
                if self.metadata.watched {
                    0.0
                } else {
                    self.metadata.current
                },
                self.path.display()
            )
        } else {
            format!(
                "--start={} \"{}\"",
                if self.metadata.watched {
                    0.0
                } else {
                    self.metadata.current
                },
                self.path.display()
            )
        };

        Backend::run_mpv(&cmd)?;
        self.update_metadata()
    }
}

impl Meta for Episode {
    fn thumbnail(&self) -> Option<PathBuf> {
        Some(self.thumbnail_path.clone())
    }

    fn description(&self) -> String {
        format!(
            "Name: {}\n\nDuration: {}{}\nWatched: {}",
            self.name,
            VideoMetadata::to_time(self.metadata.duration),
            if !self.metadata.watched && self.metadata.current > 1.0 {
                format!(
                    "\nCurrent: {}",
                    VideoMetadata::to_time(self.metadata.current)
                )
            } else {
                "".to_string()
            },
            if self.metadata.watched { "Yes" } else { "No" },
        )
    }
}
