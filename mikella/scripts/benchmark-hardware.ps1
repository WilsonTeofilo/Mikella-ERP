<#
.SYNOPSIS
    Mede latência do Mikella ERP no hardware alvo (i3/i5 de 5ª geração) e
    registra o relatório de homologação (PERF-001/PERF-002).

.DESCRIPTION
    Coleta a ficha da máquina, mede o tempo de subida do servidor (opcional) e
    a latência p50/p95 de endpoints reais. A visualização do `clip-path` é
    medida à parte, no WebView, com o roteiro impresso no relatório.

    Nada aqui grava venda: os endpoints medidos são de leitura e simulação.
    O caminho de escrita (finalizar venda) deve ser medido em horário de teste,
    com o checklist manual do fim do relatório.

.PARAMETER BaseUrl
    URL do servidor local. Padrão: http://127.0.0.1:47823

.PARAMETER Token
    Token Bearer já obtido (começa com `mikella_`). Evita login/2FA no script.

.PARAMETER Usuario
    Usuário para login. Use com -Senha ou com MIKELLA_DESATIVAR_2FA=1.

.PARAMETER Senha
    Senha do usuário. Não é gravada no relatório.

.PARAMETER Repeticoes
    Número de amostras por endpoint. Padrão: 20.

.PARAMETER Executavel
    Caminho opcional do store-server. Se informado, o script inicia o processo,
    mede o tempo até /v1/health responder e encerra ao final.

.PARAMETER Saida
    Relatório Markdown. Padrão: relatorio-homologacao-hardware.md

.PARAMETER JsonSaida
    Relatório JSON. Padrão: relatorio-homologacao-hardware.json

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/benchmark-hardware.ps1 -Usuario admin -Senha "..." -Repeticoes 30
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = "http://127.0.0.1:47823",
    [string]$Token = "",
    [string]$Usuario = "",
    [string]$Senha = "",
    [int]$Repeticoes = 20,
    [string]$Executavel = "",
    [string]$Saida = "relatorio-homologacao-hardware.md",
    [string]$JsonSaida = "relatorio-homologacao-hardware.json"
)

$ErrorActionPreference = "Stop"
$BaseUrl = $BaseUrl.TrimEnd("/")

function Mediana([double[]]$valores) {
    if ($valores.Count -eq 0) { return 0 }
    $ordenado = $valores | Sort-Object
    $meio = [int][Math]::Floor($ordenado.Count / 2)
    if ($ordenado.Count % 2 -eq 0) {
        return [Math]::Round(($ordenado[$meio - 1] + $ordenado[$meio]) / 2, 2)
    }
    return [Math]::Round($ordenado[$meio], 2)
}

function Percentil([double[]]$valores, [double]$p) {
    if ($valores.Count -eq 0) { return 0 }
    $ordenado = @($valores | Sort-Object)
    $indice = [Math]::Ceiling(($p / 100.0) * $ordenado.Count) - 1
    $indice = [Math]::Max(0, [Math]::Min($ordenado.Count - 1, $indice))
    return [Math]::Round($ordenado[$indice], 2)
}

function Estatisticas([double[]]$amostras) {
    [pscustomobject]@{
        amostras = $amostras.Count
        min_ms   = if ($amostras.Count) { [Math]::Round(($amostras | Measure-Object -Minimum).Minimum, 2) } else { 0 }
        p50_ms   = Mediana $amostras
        p95_ms   = Percentil $amostras 95
        max_ms   = if ($amostras.Count) { [Math]::Round(($amostras | Measure-Object -Maximum).Maximum, 2) } else { 0 }
        media_ms = if ($amostras.Count) { [Math]::Round(($amostras | Measure-Object -Average).Average, 2) } else { 0 }
    }
}

function Requisicao([string]$Metodo, [string]$Caminho, [hashtable]$Headers = @{}) {
    $params = @{
        Uri                = "$BaseUrl$Caminho"
        Method             = $Metodo
        UseBasicParsing    = $true
        TimeoutSec         = 30
        Headers            = $Headers
    }
    return Invoke-WebRequest @params
}

function MedirEndpoint([string]$Nome, [string]$Caminho, [hashtable]$Headers) {
    $amostras = New-Object System.Collections.Generic.List[double]
    # Aquecimento: a primeira chamada paga cache/JIT e não entra na conta.
    $null = Requisicao "GET" $Caminho $Headers
    for ($i = 0; $i -lt $Repeticoes; $i++) {
        $cronometro = [System.Diagnostics.Stopwatch]::StartNew()
        $resposta = Requisicao "GET" $Caminho $Headers
        $cronometro.Stop()
        if ($resposta.StatusCode -ge 400) {
            throw "Endpoint $Caminho respondeu $($resposta.StatusCode)"
        }
        $amostras.Add($cronometro.Elapsed.TotalMilliseconds)
    }
    $stats = Estatisticas $amostras.ToArray()
    [pscustomobject]@{ endpoint = $Nome; caminho = $Caminho; latencia = $stats }
}

# ─── Ficha da máquina ────────────────────────────────────────────────────────
Write-Host "Coletando ficha do hardware..." -ForegroundColor Cyan
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$ram = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
$discos = @()
try {
    $discos = Get-PhysicalDisk | Select-Object FriendlyName, MediaType, @{N = "TamanhoGB"; E = { [Math]::Round($_.Size / 1GB) } }
} catch {
    # Get-PhysicalDisk não existe em algumas edições do Windows.
}

$hardware = [ordered]@{
    maquina              = $env:COMPUTERNAME
    data                 = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss zzz")
    cpu                  = $cpu.Name
    nucleos_fisicos      = $cpu.NumberOfCores
    nucleos_logicos      = $cpu.NumberOfLogicalProcessors
    clock_max_mhz        = $cpu.MaxClockSpeed
    memoria_gb           = [Math]::Round($ram.TotalPhysicalMemory / 1GB, 1)
    so                   = "$($os.Caption) $($os.Version) $($os.OSArchitecture)"
    discos               = $discos
    app_versao           = "desconhecida"
    sqlite_versao        = "desconhecida"
    integridade_banco    = "não verificada"
    cadeia_livro_caixa   = "não verificada"
    cadeia_auditoria     = "não verificada"
    inicio_servidor_ms   = $null
}

# ─── Subida opcional do servidor ─────────────────────────────────────────────
$processo = $null
try {
    if ($Executavel) {
        if (-not (Test-Path -LiteralPath $Executavel)) {
            throw "Executável não encontrado: $Executavel"
        }
        Write-Host "Iniciando servidor para medir o boot..." -ForegroundColor Cyan
        $cronometroBoot = [System.Diagnostics.Stopwatch]::StartNew()
        $processo = Start-Process -FilePath $Executavel -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput "$env:TEMP\mikella-bench-out.log" `
            -RedirectStandardError "$env:TEMP\mikella-bench-err.log"
        $limite = (Get-Date).AddSeconds(90)
        $subiu = $false
        while ((Get-Date) -lt $limite) {
            Start-Sleep -Milliseconds 100
            try {
                $null = Requisicao "GET" "/v1/health"
                $subiu = $true
                break
            } catch { }
        }
        $cronometroBoot.Stop()
        if (-not $subiu) {
            throw "O servidor não respondeu /v1/health em 90s."
        }
        $hardware.inicio_servidor_ms = [Math]::Round($cronometroBoot.Elapsed.TotalMilliseconds, 2)
        Write-Host ("Boot medido: {0} ms" -f $hardware.inicio_servidor_ms) -ForegroundColor Green
    }

    # ─── Autenticação ───────────────────────────────────────────────────────
    if (-not $Token) {
        if (-not $Usuario -or -not $Senha) {
            throw "Informe -Token, ou -Usuario e -Senha, para medir os endpoints protegidos."
        }
        Write-Host "Autenticando..." -ForegroundColor Cyan
        $login = Invoke-WebRequest -Uri "$BaseUrl/v1/login" -Method POST `
            -UseBasicParsing -TimeoutSec 30 `
            -Body @{ username = $Usuario; senha = $Senha }
        $corpo = $login.Content | ConvertFrom-Json
        if ($corpo.token) {
            $Token = $corpo.token
        } elseif ($corpo.codigo_dev) {
            $verificacao = Invoke-WebRequest -Uri "$BaseUrl/v1/2fa/verificar" -Method POST `
                -UseBasicParsing -TimeoutSec 30 `
                -Body @{ username = $corpo.username; codigo = $corpo.codigo_dev }
            $Token = ($verificacao.Content | ConvertFrom-Json).token
        } else {
            throw "Login pediu 2FA sem código de desenvolvimento. Rode com MIKELLA_DESATIVAR_2FA=1 ou passe -Token."
        }
        if (-not $Token) { throw "Login não devolveu token." }
    }
    $headers = @{ Authorization = "Bearer $Token" }

    # ─── Diagnóstico do banco ───────────────────────────────────────────────
    Write-Host "Lendo diagnóstico..." -ForegroundColor Cyan
    $diag = (Requisicao "GET" "/v1/sistema/diagnostico" $headers).Content | ConvertFrom-Json
    $hardware.app_versao = $diag.app_versao
    $hardware.sqlite_versao = $diag.banco.sqlite_versao
    $hardware.integridade_banco = $diag.banco.integrity_check
    $hardware.cadeia_livro_caixa = $diag.banco.cadeia_hash_livro_caixa_integra
    $hardware.cadeia_auditoria = $diag.banco.cadeia_hash_auditoria_integra

    # ─── Latências ──────────────────────────────────────────────────────────
    $endpoints = @(
        @{ nome = "Health";             caminho = "/v1/health" },
        @{ nome = "Produtos (lista)";   caminho = "/v1/produtos" },
        @{ nome = "Vendas (histórico)"; caminho = "/v1/vendas?limite=50&sem_filtro_data=true" },
        @{ nome = "Clientes";           caminho = "/v1/clientes" },
        @{ nome = "Devedores";          caminho = "/v1/crediario/devedores" },
        @{ nome = "Dashboard";          caminho = "/v1/finance/resumo" },
        @{ nome = "Fluxo de caixa";     caminho = "/v1/finance/fluxo-caixa" },
        @{ nome = "DRE";                caminho = "/v1/finance/resultado" },
        @{ nome = "Diagnóstico";        caminho = "/v1/sistema/diagnostico" },
        @{ nome = "Simular parcelas";   caminho = "/v1/vendas/simular-parcelas?valor_centavos=10000&parcelas=3" }
    )

    $resultados = New-Object System.Collections.Generic.List[object]
    foreach ($item in $endpoints) {
        Write-Host ("Medindo {0}..." -f $item.nome) -ForegroundColor Cyan
        try {
            $resultados.Add((MedirEndpoint $item.nome $item.caminho $headers))
        } catch {
            Write-Host ("  ignorado: {0}" -f $_.Exception.Message) -ForegroundColor Yellow
        }
    }

    # ─── Relatório ──────────────────────────────────────────────────────────
    $md = New-Object System.Collections.Generic.List[string]
    $md.Add("# Relatório de homologação de hardware — Mikella ERP")
    $md.Add("")
    $md.Add("Gerado em **$($hardware.data)** em ``$($hardware.maquina)``.")
    $md.Add("")
    $md.Add("## Ficha da máquina")
    $md.Add("")
    $md.Add("| Item | Valor |")
    $md.Add("|---|---|")
    $md.Add("| CPU | $($hardware.cpu) |")
    $md.Add("| Núcleos físicos / lógicos | $($hardware.nucleos_fisicos) / $($hardware.nucleos_logicos) |")
    $md.Add("| Clock máximo | $($hardware.clock_max_mhz) MHz |")
    $md.Add("| Memória RAM | $($hardware.memoria_gb) GB |")
    $md.Add("| Sistema | $($hardware.so) |")
    foreach ($d in $hardware.discos) {
        $md.Add("| Disco | $($d.FriendlyName) — $($d.MediaType) — $($d.TamanhoGB) GB |")
    }
    $md.Add("| Versão do app | $($hardware.app_versao) |")
    $md.Add("| SQLite | $($hardware.sqlite_versao) |")
    $md.Add("| integrity_check | $($hardware.integridade_banco) |")
    $md.Add("| Cadeia livro-caixa | $($hardware.cadeia_livro_caixa) |")
    $md.Add("| Cadeia auditoria | $($hardware.cadeia_auditoria) |")
    if ($null -ne $hardware.inicio_servidor_ms) {
        $md.Add("| Boot até /v1/health | $($hardware.inicio_servidor_ms) ms |")
    }
    $md.Add("")
    $md.Add("## Latência dos endpoints (ms)")
    $md.Add("")
    $md.Add("| Endpoint | Amostras | min | p50 | p95 | max | média |")
    $md.Add("|---|---:|---:|---:|---:|---:|---:|")
    foreach ($r in $resultados) {
        $l = $r.latencia
        $md.Add("| $($r.endpoint) | $($l.amostras) | $($l.min_ms) | $($l.p50_ms) | $($l.p95_ms) | $($l.max_ms) | $($l.media_ms) |")
    }
    $md.Add("")
    $md.Add("### Metas do plano")
    $md.Add("")
    $md.Add("- PERF-001: abertura de tela operacional **p95 ≤ 1.500 ms**.")
    $md.Add("- PERF-002: finalização de venda **p95 ≤ 800 ms** no backend (medir no horário de teste).")
    $md.Add("- Sem travar a UI: nenhuma tarefa de banco acima de **50 ms** na thread do WebView.")
    $md.Add("")
    $md.Add("## Roteiro manual: `clip-path` e fluidez (WebView2)")
    $md.Add("")
    $md.Add("1. Abra o app empacotado (`dist/`) nesta máquina, não o preview.")
    $md.Add("2. Abra DevTools (F12) → Performance → grave 10 s navegando em `pos.html` e `main-menu.html`.")
    $md.Add("3. Confirme que quadros ficam **≥ 50 fps** e que não há *long task* > 50 ms.")
    $md.Add("4. Em Rendering, ligue **Paint flashing** e **Frame Rendering Stats**;")
    $md.Add("   o corte do bloco de total (`clip-path`) não deve repintar a tela inteira.")
    $md.Add("5. Repita com 30 abas abertas no navegador para simular a máquina do lojista.")
    $md.Add("")
    $md.Add("## Checklist de fechamento")
    $md.Add("")
    $md.Add("- [ ] Rodado no i3/i5 de 5ª geração alvo, com 4 GB ou 8 GB de RAM.")
    $md.Add("- [ ] `integrity_check = ok` e as duas cadeias de hash íntegras.")
    $md.Add("- [ ] Venda de teste finalizada dentro do p95 de PERF-002.")
    $md.Add("- [ ] Sem jank perceptível no `clip-path` (vídeo ou gravação do Performance).")
    $md.Add("- [ ] Relatório anexado à entrega da fase F8.")

    $md -join "`n" | Set-Content -LiteralPath $Saida -Encoding UTF8

    $json = [ordered]@{
        hardware   = $hardware
        endpoints  = @($resultados | ForEach-Object {
            [ordered]@{
                endpoint = $_.endpoint
                caminho  = $_.caminho
                latencia = $_.latencia
            }
        })
    }
    $json | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $JsonSaida -Encoding UTF8

    Write-Host ""
    Write-Host "Relatório gerado:" -ForegroundColor Green
    Write-Host "  $Saida"
    Write-Host "  $JsonSaida"
    $md | ForEach-Object { Write-Host $_ }
}
finally {
    if ($processo -and -not $processo.HasExited) {
        Stop-Process -Id $processo.Id -Force -ErrorAction SilentlyContinue
    }
}
