#[test]
fn background_command_targets_the_requested_program() {
    let command = ustt_media::process::command("ffmpeg");
    assert_eq!(command.get_program(), "ffmpeg");
}
