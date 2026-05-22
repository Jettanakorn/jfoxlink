# Generate NACA0018 airfoil STL for VAWT blade
$outFile = "D:\naca0018_vawt.stl"
$chord = 0.3; $span = 1.0; $nPts = 60

function Thick($x) { return (0.18/0.20)*(0.2969*[Math]::Sqrt($x)-0.1260*$x-0.3516*$x*$x+0.2843*$x*$x*$x-0.1015*$x*$x*$x*$x) }

# Generate full profile: upper LE->TE, then lower TE->LE
$profile = @()
for ($i = 0; $i -le $nPts; $i++) { $x=$i/$nPts; $yt=Thick($x); $profile+=@($x*$chord, $yt*$chord) }
for ($i = $nPts; $i -ge 0; $i--)   { $x=$i/$nPts; $yt=Thick($x); $profile+=@($x*$chord, -$yt*$chord) }

$tris = @()
$n = $nPts*2+2  # total profile points

# Extrude profile from z=0 to z=$span
for ($i = 0; $i -lt $n-1; $i++) {
    $j = ($i+1) % $n
    $ax=$profile[$i*2];   $ay=$profile[$i*2+1]
    $bx=$profile[$j*2];   $by=$profile[$j*2+1]
    # Front face (z=0)
    $tris+=@($ax,$ay,0, $bx,$by,0, $ax,$ay,$span)
    # Back face (z=$span, reversed winding)
    $tris+=@($ax,$ay,$span, $bx,$by,0, $bx,$by,$span)
}

# Write binary STL
$stream = [System.IO.File]::OpenWrite($outFile)
$writer = [System.IO.BinaryWriter]::new($stream)
$writer.Write([byte[]]::new(80))
$count = $tris.Count / 9
$writer.Write([uint32]$count)

for ($i = 0; $i -lt $tris.Count; $i += 9) {
    $ax,$ay,$az,$bx,$by,$bz,$cx,$cy,$cz = $tris[$i..($i+8)]
    # Normal
    $nx = ($ay-$cy)*($bz-$cz)-($az-$cz)*($by-$cy)
    $ny = ($az-$cz)*($bx-$cx)-($ax-$cx)*($bz-$cz)
    $nz = ($ax-$cx)*($by-$cy)-($ay-$cy)*($bx-$cx)
    $len = [Math]::Sqrt($nx*$nx+$ny*$ny+$nz*$nz)
    if ($len -gt 0) { $nx/=$len; $ny/=$len; $nz/=$len }
    $writer.Write([single]$nx);$writer.Write([single]$ny);$writer.Write([single]$nz)
    $writer.Write([single]$ax);$writer.Write([single]$ay);$writer.Write([single]$az)
    $writer.Write([single]$bx);$writer.Write([single]$by);$writer.Write([single]$bz)
    $writer.Write([single]$cx);$writer.Write([single]$cy);$writer.Write([single]$cz)
    $writer.Write([uint16]0)
}
$writer.Close(); $stream.Close()
Write-Host "✅ NACA0018 VAWT blade: $outFile ($count triangles)" -ForegroundColor Green
