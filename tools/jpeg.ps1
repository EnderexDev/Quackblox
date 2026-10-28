# Converts PNG captures to JPEG (quality 78) beside them: tools/jpeg.ps1 a.png b.png
Add-Type -AssemblyName System.Drawing
$codec = [Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
$params = New-Object Drawing.Imaging.EncoderParameters 1
$params.Param[0] = New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality, [long]78)
foreach ($f in $args) {
  $full = (Resolve-Path $f).Path
  $img = [Drawing.Image]::FromFile($full)
  $img.Save([IO.Path]::ChangeExtension($full, '.jpg'), $codec, $params)
  $img.Dispose()
}
