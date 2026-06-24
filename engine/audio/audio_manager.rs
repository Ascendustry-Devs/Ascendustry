use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};
use std::io::Cursor;
use winit::window::Window;

pub struct GameAudioManager {
    manager: AudioManager,
    music_track: TrackHandle,
    main_theme_handle: Option<StreamingSoundHandle<kira::sound::FromFileError>>,
}

impl GameAudioManager {
    pub fn new(window: &Window) -> anyhow::Result<Self> {
        let _ = window;
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;

        let music_track = manager.add_sub_track(TrackBuilder::new())?;

        Ok(Self {
            manager,
            music_track,
            main_theme_handle: None,
        })
    }

    pub fn play_main_theme(&mut self) -> anyhow::Result<()> {
        if self.main_theme_handle.is_some() {
            return Ok(());
        }

        let main_theme_data = include_bytes!("../../assets/sounds/main_theme.mp3");
        let cursor = Cursor::new(main_theme_data.as_slice());
        let sound_data = StreamingSoundData::from_cursor(cursor)?;

        let handle = self.music_track.play(sound_data)?;
        self.main_theme_handle = Some(handle);

        Ok(())
    }

    pub fn stop_main_theme(&mut self) {
        if let Some(mut handle) = self.main_theme_handle.take() {
            handle.stop(Tween::default());
        }
    }

    pub fn set_music_volume(&mut self, volume_db: f32) {
        self.music_track.set_volume(Decibels::from(volume_db), Tween::default());
    }

    pub fn update(&mut self) {
        let _ = self.manager.backend_mut();
    }
}
