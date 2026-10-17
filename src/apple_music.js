function run(argv) {
    const command = argv[0] || 'read';
    const music = Application('Music');
    if (!music.running()) {
        return JSON.stringify({track: null, status: 'Open Apple Music and play a track'});
    }
    if (command === 'previous') music.previousTrack();
    else if (command === 'toggle') music.playpause();
    else if (command === 'next') music.nextTrack();
    else if (command !== 'read') throw new Error('Unsupported music command');
    const state = music.playerState();
    if (state !== 'playing' && state !== 'paused') {
        return JSON.stringify({track: null, status: 'Choose a track in Apple Music'});
    }
    const track = music.currentTrack();

    return JSON.stringify({track: {
        title: track.name(), artist: track.artist(),
        playing: state === 'playing', position: music.playerPosition(),
        duration: track.duration()
    }, status: state === 'playing' ? 'Playing' : 'Paused'});
}
