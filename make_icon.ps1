Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 256,256
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias

# Фон - синий
$rect = New-Object System.Drawing.Rectangle 0,0,256,256
$brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(0,120,215))
$g.FillRectangle($brush, $rect)

# Круглый контейнер белый
$g.FillEllipse([System.Drawing.Brushes]::White, 32,32,192,192)

# Обводка круга синяя
$pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(0,120,215), 4)
$g.DrawEllipse($pen, 32,32,192,192)

# Буква S синяя
$font = New-Object System.Drawing.Font("Segoe UI", 130, [System.Drawing.FontStyle]::Bold)
$sf = New-Object System.Drawing.StringFormat
$sf.Alignment = [System.Drawing.StringAlignment]::Center
$sf.LineAlignment = [System.Drawing.StringAlignment]::Center
$sBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(0,120,215))
$g.DrawString("S", $font, $sBrush, 128, 128, $sf)

# Точка внизу справа красная
$g.FillEllipse([System.Drawing.Brushes]::Red, 200,200,32,32)

$g.Dispose()
$pen.Dispose()
$font.Dispose()
$sBrush.Dispose()
$brush.Dispose()
$bmp.Save("E:\site\IDM\SDA/resources/app.ico", [System.Drawing.Imaging.ImageFormat]::Icon)
$bmp.Dispose()
