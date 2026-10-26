param([Parameter(Mandatory)][string]$OutputPath)
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath $OutputPath) { throw 'Use a fresh output path' }
Add-Type -AssemblyName System.Drawing
$collection=[Drawing.Text.InstalledFontCollection]::new()
try {
    $families=@($collection.Families | ForEach-Object Name | Sort-Object -Unique)
    $files=@()
    foreach ($directory in @((Join-Path $env:WINDIR 'Fonts'),(Join-Path $env:LOCALAPPDATA 'Microsoft/Windows/Fonts'))) {
        if (Test-Path $directory) {
            foreach ($file in Get-ChildItem -LiteralPath $directory -File) {
                $files+=@{path=$file.FullName;sha256=(Get-FileHash -LiteralPath $file.FullName).Hash}
            }
        }
    }
    @{schema_version=1;utc=[DateTime]::UtcNow.ToString('o');installed_gdi_families=$families;font_files=$files;assignment_evidence='office.json Font.Name reports assigned COM font only';actual_rendered_substitution='unverified: installed family presence and assigned name do not identify glyph fallback'} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutputPath
} finally { $collection.Dispose() }
