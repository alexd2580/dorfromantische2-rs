use std::{
    path::{Path, PathBuf},
    thread::JoinHandle,
    time::{Duration, SystemTime},
};

use crate::{
    best_placements::BestPlacements, group_assignments::GroupAssignments, map::Map, raw_data,
};

use std::fs::File;

#[derive(Default)]
pub struct FileChooseDialog {
    handle: Option<JoinHandle<Option<PathBuf>>>,
}

impl FileChooseDialog {
    pub fn is_open(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn open(&mut self) {
        if !self.is_open() {
            self.handle = Some(std::thread::spawn(|| {
                rfd::FileDialog::new().set_directory(".").pick_file()
            }));
        }
    }

    fn take_result(&mut self) -> Option<PathBuf> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            self.handle
                .take()
                .unwrap()
                .join()
                .expect("Failed to choose file")
        } else {
            None
        }
    }
}

#[derive(Default)]
pub struct MapLoader {
    handle: Option<JoinHandle<(Map, GroupAssignments, BestPlacements)>>,
}

impl MapLoader {
    pub fn in_progress(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    fn load(&mut self, path: &Path) {
        if !self.in_progress() {
            let path = path.to_owned();
            self.handle = Some(std::thread::spawn(|| {
                // Load savegame.
                let path = path;

                log::info!("Loading savegame: {}", path.display());
                let start = std::time::Instant::now();

                // Fugly retry mechanism.
                let parsed = std::panic::catch_unwind(|| {
                    let mut stream = File::open(&path).expect("Failed to open file");
                    nrbf_rs::parse_nrbf(&mut stream)
                })
                .or_else(|_| {
                    std::panic::catch_unwind(|| {
                        let mut stream = File::open(&path).expect("Failed to open file");
                        nrbf_rs::parse_nrbf(&mut stream)
                    })
                })
                .or_else(|_| {
                    std::panic::catch_unwind(|| {
                        let mut stream = File::open(&path).expect("Failed to open file");
                        nrbf_rs::parse_nrbf(&mut stream)
                    })
                })
                .unwrap();

                let tree_loaded = start.elapsed();
                println!("NRBF Tree loaded in: {tree_loaded:?}");
                let start = std::time::Instant::now();

                let savegame = raw_data::SaveGame::try_from(&parsed).unwrap();

                let save_loaded = start.elapsed();
                println!("Savegame loaded in: {save_loaded:?}");
                let start = std::time::Instant::now();

                let map = Map::from(&savegame);
                let groups = GroupAssignments::from(&map);
                let freqs = crate::tile_frequency::TileFrequencies::from_map(&map);
                let best_placements = BestPlacements::compute(&map, &groups, &freqs);
                let map_loaded = start.elapsed();
                println!("Map loaded in: {map_loaded:?}");

                (map, groups, best_placements)
            }));
        }
    }

    pub fn take_result(&mut self) -> Option<(Map, GroupAssignments, BestPlacements)> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            self.handle.take().unwrap().join().ok()
        } else {
            None
        }
    }
}

fn previous_file_path_cache_path() -> PathBuf {
    let mut previous_file_path =
        dirs::cache_dir().expect("There is no cache directory on this system");
    previous_file_path.push("dorfromantische2-rs/previous_file_path");
    let _ = std::fs::create_dir_all(previous_file_path.parent().unwrap());
    previous_file_path
}

pub struct FileWatcher {
    pub file_choose_dialog: FileChooseDialog,
    pub file: Option<PathBuf>,
    pub mtime: SystemTime,
    pub change_detected: bool,
    pub map_loader: MapLoader,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self {
            file_choose_dialog: FileChooseDialog::default(),
            file: None,
            mtime: SystemTime::now(),
            change_detected: false,
            map_loader: MapLoader::default(),
        }
    }
}

impl FileWatcher {
    pub fn set_file_path(&mut self, file: &Path) {
        self.file = Some(file.to_path_buf());
        self.mtime = SystemTime::UNIX_EPOCH;

        let cache_path = previous_file_path_cache_path();
        std::fs::write(cache_path, file.to_str().unwrap())
            .expect("Failed to write file path to cache");
    }

    pub fn use_previous_file_path(&mut self) {
        let cache_path = previous_file_path_cache_path();
        if let Ok(file_path) = std::fs::read_to_string(cache_path) {
            self.set_file_path(&PathBuf::from(file_path));
        }
    }

    pub fn handle_file_dialog(&mut self) {
        if let Some(file) = self.file_choose_dialog.take_result() {
            self.set_file_path(&file);
        }
    }

    pub fn reload_file_if_changed(&mut self) {
        if let Some(file) = self.file.as_ref() {
            let actual_mtime = file.metadata().ok().and_then(|md| md.modified().ok());
            if let Some(actual_mtime) = actual_mtime {
                if actual_mtime > self.mtime {
                    self.change_detected = true;
                    self.mtime = actual_mtime;
                } else if self.change_detected
                    && actual_mtime == self.mtime
                    && SystemTime::now() > actual_mtime + Duration::from_secs(1)
                {
                    self.change_detected = false;
                    self.map_loader.load(file);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_watcher_initial_state() {
        let fw = FileWatcher::default();
        assert!(fw.file.is_none());
        assert!(!fw.change_detected);
        assert!(!fw.file_choose_dialog.is_open());
        assert!(!fw.map_loader.in_progress());
    }

    #[test]
    fn test_set_file_path_stores_path() {
        let mut fw = FileWatcher::default();
        let path = PathBuf::from("/tmp/test_dorfromantische2_rs_test.sav");
        fw.file = Some(path.clone());
        fw.mtime = SystemTime::UNIX_EPOCH;
        assert_eq!(fw.file.as_ref().unwrap(), &path);
        assert_eq!(fw.mtime, SystemTime::UNIX_EPOCH);
    }
}
