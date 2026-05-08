use anyhow::Result;

pub async fn download_and_transcribe(_file_url: &str) -> Result<String> {
    // Stub: would download OGG, convert to WAV, run whisper
    anyhow::bail!("Voice transcription not implemented")
}
