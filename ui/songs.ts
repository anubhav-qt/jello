// The songs in Music/Jello/Songs and the one open
// (docs/adr/0038-song-files.md, `SongsView` in
// src-tauri/src/song_session.rs).
export interface SongsView {
  // Sorted by name, with upper and lower case together.
  songs: SongView[];
  // The open song's name.
  open: string;
}

export interface SongView {
  name: string;
  // False for a file that is not a song file: its tile is dimmed.
  readable: boolean;
}

// What the Songs panel sends to Rust. ui/main.ts sends each in order with
// the keys. Each is done once Rust has answered, or once the top bar says
// why it failed.
export interface SongSender {
  // The songs, as Rust lists them now.
  listSongs(): Promise<SongsView | null>;
  // Opens song `name`, after saving the one open.
  openSong(name: string): Promise<void>;
  // Makes a new song, after saving the one open.
  newSong(): Promise<void>;
  // Renames the open song to `name`.
  renameSong(name: string): Promise<void>;
  // Why the open song can't be renamed `name`, or null if it can.
  songNameProblem(name: string): Promise<string | null>;
}
