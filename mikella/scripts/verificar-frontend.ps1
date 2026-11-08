<#
.SYNOPSIS
    Guarda automatizado do frontend do Mikella ERP.

.DESCRIPTION
    Falha quando uma pagina HTML volta a virar monolito ou quando uma
    referencia de asset deixa de existir. Regras verificadas:

    1. Nenhum bloco <style> inline com mais de 400 caracteres.
    2. Nenhum <script> inline (sem atributo src).
    3. Nenhum handler inline on*= (onclick, onchange, onsubmit, ...).
    4. Todo href/src relativo aponta para um arquivo existente.
    5. Nenhuma pasta de fontes duplicada fora de shared/design/fontes.
    6. Nenhum `innerHTML`/`data-*` montado com interpolacao de dado dinamico
       (XSS armazenado — SEC-008).
    7. Todo modulo JS referenciado passa em `node --check` (se node existir).

.PARAMETER Raiz
    Pasta de origem do frontend. Padrao: ../frontend/src relativo ao script.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/verificar-frontend.ps1
#>
[CmdletBinding()]
param(
    [string]$Raiz
)

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Raiz) { $Raiz = Join-Path $scriptDir '..\frontend\src' }
$Raiz = (Resolve-Path $Raiz).Path

$limiteCssInline = 400
$falhas = New-Object System.Collections.Generic.List[string]

function Registrar-Falha([string]$msg) {
    $script:falhas.Add($msg)
    Write-Host "FALHA  $msg" -ForegroundColor Red
}

function Registrar-Ok([string]$msg) {
    Write-Host "ok     $msg" -ForegroundColor DarkGray
}

Write-Host "Verificando frontend em: $Raiz" -ForegroundColor Cyan

$htmls = Get-ChildItem -Path $Raiz -Recurse -File -Filter *.html
$modulos = Get-ChildItem -Path $Raiz -Recurse -File -Include *.js -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch '\\node_modules\\' }

foreach ($arquivo in $htmls) {
    $rel = $arquivo.FullName.Substring($Raiz.Length).TrimStart('\')
    $texto = Get-Content -LiteralPath $arquivo.FullName -Raw

    # 1) blocos <style> longos
    foreach ($m in [regex]::Matches($texto, '(?s)<style[^>]*>(.*?)</style>')) {
        $tamanho = $m.Groups[1].Value.Trim().Length
        if ($tamanho -gt $limiteCssInline) {
            Registrar-Falha "$rel tem bloco <style> inline com $tamanho caracteres (limite $limiteCssInline). Mova para features/<feature>/*.css."
        }
    }

    # 2) scripts inline
    foreach ($m in [regex]::Matches($texto, '(?s)<script(?![^>]*\bsrc=)[^>]*>(.*?)</script>')) {
        if ($m.Groups[1].Value.Trim().Length -gt 0) {
            Registrar-Falha "$rel tem <script> inline. Extraia para um modulo em features/ ou shared/."
        }
    }

    # 3) handlers inline
    foreach ($m in [regex]::Matches($texto, '\son[a-z]+\s*=')) {
        Registrar-Falha "$rel usa handler inline '$($m.Value.Trim())'. Use addEventListener no modulo da feature."
    }

    # 4) referencias relativas
    foreach ($m in [regex]::Matches($texto, '(?:href|src)="([^"]+)"')) {
        $url = $m.Groups[1].Value
        if ($url -match '^(https?:|#|data:|mailto:)') { continue }
        $caminhoLimpo = ($url -split '[?#]')[0]
        $alvo = Join-Path $arquivo.DirectoryName $caminhoLimpo
        if (-not (Test-Path -LiteralPath $alvo)) {
            Registrar-Falha "$rel referencia asset inexistente: $url"
        }
    }

    Registrar-Ok "$rel"
}

# 5) fontes duplicadas
$pastasFontes = Get-ChildItem -Path $Raiz -Recurse -Directory -Filter fontes -ErrorAction SilentlyContinue
if ($pastasFontes.Count -gt 1) {
    $locais = ($pastasFontes | ForEach-Object { $_.FullName.Substring($Raiz.Length).TrimStart('\') }) -join ', '
    Registrar-Falha "fontes duplicadas em: $locais (mantenha apenas shared/design/fontes)."
}

# 6) innerHTML com dado dinamico (XSS armazenado — SEC-008)
foreach ($modulo in $modulos) {
    $linhas = Get-Content -LiteralPath $modulo.FullName
    for ($i = 0; $i -lt $linhas.Count; $i++) {
        $linha = $linhas[$i]
        if ($linha -match 'innerHTML\s*=' -and ($linha -match '\$\{' -or $linha -match '\+')) {
            Registrar-Falha "$($modulo.Name):$($i + 1) atribui innerHTML com interpolacao/concatencao. Use textContent/createElement (SEC-008)."
        }
        if ($linha -match 'data-[a-z-]+\s*=\s*[''"][^''"]*\$\{') {
            Registrar-Falha "$($modulo.Name):$($i + 1) monta atributo data-* com interpolacao. Guarde o objeto em closure (SEC-008)."
        }
    }
}

# 7) sintaxe dos modulos JS
$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
    $temp = Join-Path ([System.IO.Path]::GetTempPath()) ("mikella-syntax-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    try {
        foreach ($modulo in $modulos) {
            $copia = Join-Path $temp ($modulo.BaseName + '-' + [guid]::NewGuid().ToString('N').Substring(0,6) + '.mjs')
            Copy-Item -LiteralPath $modulo.FullName -Destination $copia -Force
            $saida = & node --check $copia 2>&1
            if ($LASTEXITCODE -ne 0) {
                Registrar-Falha "sintaxe JS invalida em $($modulo.Name): $saida"
            }
            Remove-Item -LiteralPath $copia -Force
        }
        Registrar-Ok "sintaxe de $($modulos.Count) modulo(s) JS"
    }
    finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
else {
    Write-Host "aviso  node nao encontrado; verificacao de sintaxe JS ignorada." -ForegroundColor Yellow
}

Write-Host ""
if ($falhas.Count -gt 0) {
    Write-Host "Frontend REPROVADO: $($falhas.Count) problema(s)." -ForegroundColor Red
    exit 1
}

Write-Host "Frontend aprovado: sem monolitos, sem handlers inline, referencias OK." -ForegroundColor Green
exit 0
