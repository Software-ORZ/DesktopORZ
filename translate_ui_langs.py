#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# DesktopORZ - Save and restore Windows desktop icon layouts
# Copyright (C) 2026 ThainanViniciusKatchan
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program. If not, see <https://www.gnu.org/licenses/>.
"""
Ferramenta de i18n do DesktopORZ (`langs/ui_langs/`).

O `en-us.json` é a referência (é o idioma de fallback lido por `src/i18n.rs`);
os outros idiomas precisam ter as mesmas chaves. Esta ferramenta:

  * confere a paridade de chaves de todos os arquivos (modo offline);
  * completa as chaves que faltam pedindo a tradução ao gateway hospedado da
    Respan (https://api.respan.ai/api), usando `RESPAN_API_KEY`.

Uso:
    python translate_ui_langs.py            # confere a paridade (offline)
    python translate_ui_langs.py --translate
    python translate_ui_langs.py --translate --lang es,zh --dry-run

Sem `RESPAN_API_KEY` o modo de conferência continua funcionando normalmente;
apenas a tradução automática fica indisponível.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Iterable

PROJECT_DIR = Path(__file__).resolve().parent
UI_LANGS_DIR = PROJECT_DIR / "langs" / "ui_langs"
ENV_FILE = PROJECT_DIR / ".env"

BASE_LANGUAGE = "en-us"
API_KEY_VAR = "RESPAN_API_KEY"
MODEL_VAR = "RESPAN_MODEL"
DEFAULT_MODEL = "gpt-4o-mini"

# Gateway hospedado da Respan (compatível com o formato OpenAI).
GATEWAY_URL = "https://api.respan.ai/api/chat/completions"

# Quantas chaves vão em cada pedido ao modelo.
BATCH_SIZE = 60
REQUEST_TIMEOUT = 120

PLACEHOLDER = re.compile(r"\{[A-Za-z0-9_]+\}")

LANGUAGE_NAMES = {
    "pt-br": "português do Brasil",
    "es": "espanhol",
    "ru": "russo",
    "zh": "chinês simplificado",
    "en-us": "inglês (Estados Unidos)",
}

SYSTEM_PROMPT = (
    "Você traduz os textos da interface do DesktopORZ, um utilitário Windows que "
    "salva e restaura a disposição dos ícones da área de trabalho. Traduza de forma "
    "curta e natural, respeitando o papel de cada string (título, rótulo, botão, "
    "aviso, item de menu). Preserve exatamente os placeholders entre chaves, os "
    "emojis, a pontuação final e as flags de linha de comando (--kb, --mou, "
    "save-res). Não traduza nomes próprios nem caminhos de arquivo."
)


# ---------------------------------------------------------------------------
# AMBIENTE
# ---------------------------------------------------------------------------


def load_env_file() -> None:
    """Lê `CHAVE=valor` do `.env` da raiz sem sobrescrever o ambiente atual."""
    if not ENV_FILE.is_file():
        return
    for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        name, value = line.split("=", 1)
        name = name.strip()
        value = value.strip().strip('"').strip("'")
        if name:
            os.environ.setdefault(name, value)


def gateway_credentials() -> tuple[str, str]:
    """Retorna (chave, modelo) do ambiente. A chave nunca é impressa."""
    api_key = os.environ.get("RESPAN_API_KEY", "").strip()
    model = os.environ.get(MODEL_VAR, "").strip() or DEFAULT_MODEL
    return api_key, model


# ---------------------------------------------------------------------------
# LEITURA DOS ARQUIVOS DE IDIOMA
# ---------------------------------------------------------------------------


def read_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def flatten(value: Any, prefix: str, out: dict[str, str]) -> None:
    """Achata o JSON em chaves `secao.chave`, igual ao `flatten` de `src/i18n.rs`."""
    if isinstance(value, dict):
        for key, child in value.items():
            flatten(child, f"{prefix}.{key}" if prefix else key, out)
    elif isinstance(value, str):
        out[prefix] = value


def flatten_file(path: Path) -> dict[str, str]:
    flat: dict[str, str] = {}
    flatten(read_json(path), "", flat)
    return flat


def missing_keys(base: dict[str, str], target: dict[str, str]) -> list[str]:
    """Chaves da referência que não existem (ou estão vazias) no idioma alvo."""
    return [key for key in base if not target.get(key, "").strip()]


def extra_keys(base: dict[str, str], target: dict[str, str]) -> list[str]:
    return [key for key in target if key not in base]


def set_nested(root: dict[str, Any], dotted: str, value: str) -> None:
    """Grava `secao.sub.chave` dentro do JSON aninhado, preservando a ordem."""
    *parents, leaf = dotted.split(".")
    node = root
    for part in parents:
        child = node.get(part)
        if not isinstance(child, dict):
            child = {}
            node[part] = child
        node = child
    node[leaf] = value


def write_json(path: Path, data: dict[str, Any]) -> None:
    text = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    path.write_text(text, encoding="utf-8", newline="\n")


def language_codes() -> list[str]:
    """Siglas dos idiomas de interface, exceto a referência."""
    codes = sorted(
        path.stem for path in UI_LANGS_DIR.glob("*.json") if path.stem != BASE_LANGUAGE
    )
    return codes


# ---------------------------------------------------------------------------
# GATEWAY DA RESPAN
# ---------------------------------------------------------------------------


def placeholders(text: str) -> set[str]:
    return set(PLACEHOLDER.findall(text))


def build_payload(keys: Iterable[str], base: dict[str, str], code: str, model: str) -> dict[str, Any]:
    strings = {key: base[key] for key in keys}
    instrucao = (
        f"Traduza os textos da interface para {LANGUAGE_NAMES.get(code, code)}. "
        "Responda apenas com um objeto JSON {chave: tradução}, com todas as chaves "
        "recebidas e sem comentários."
    )
    return {
        "model": model,
        "temperature": 0,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {
                "role": "user",
                "content": instrucao
                + "\n\n"
                + json.dumps({"language": code, "strings": strings}, ensure_ascii=False, indent=2),
            },
        ],
    }


def post_gateway(payload: dict[str, Any], api_key: str) -> dict[str, Any]:
    request = urllib.request.Request(
        GATEWAY_URL,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT) as response:
        return json.loads(response.read().decode("utf-8"))


def extract_json_object(text: str) -> dict[str, Any]:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```[A-Za-z]*\s*", "", text)
        text = re.sub(r"\s*```$", "", text)
    start, end = text.find("{"), text.rfind("}")
    if start == -1 or end <= start:
        raise ValueError("a resposta do gateway não contém um objeto JSON")
    return json.loads(text[start : end + 1])


def translation_problem(source: str, translated: Any) -> str | None:
    """Motivo para recusar uma tradução (ou `None` se estiver tudo certo)."""
    if not isinstance(translated, str) or not translated.strip():
        return "tradução vazia"
    if placeholders(source) != placeholders(translated):
        return "placeholders diferentes do original"
    return None


def translate_batch(
    keys: list[str],
    base: dict[str, str],
    code: str,
    model: str,
    api_key: str,
) -> dict[str, str]:
    payload = build_payload(keys, base, code, model)
    data = post_gateway(payload, api_key)

    choices = data.get("choices") or []
    if not choices:
        raise ValueError(f"resposta inesperada do gateway: {json.dumps(data, ensure_ascii=False)[:300]}")
    content = (choices[0].get("message") or {}).get("content") or ""
    try:
        traducoes = extract_json_object(content)
    except ValueError as error:
        raise ValueError(f"{error}: {content[:300]}") from error

    accepted: dict[str, str] = {}
    for key in keys:
        problem = translation_problem(base[key], traducoes.get(key))
        if problem:
            print(f"  [{code}] {key}: ignorada ({problem})", file=sys.stderr)
            continue
        accepted[key] = traducoes[key].strip()
    return accepted


def chunks(items: list[str], size: int) -> Iterable[list[str]]:
    for start in range(0, len(items), size):
        yield items[start : start + size]


# ---------------------------------------------------------------------------
# COMANDOS
# ---------------------------------------------------------------------------


def run_check(codes: list[str]) -> int:
    base = flatten_file(UI_LANGS_DIR / f"{BASE_LANGUAGE}.json")
    print(f"Referência: langs/ui_langs/{BASE_LANGUAGE}.json ({len(base)} chaves)")

    total_faltando = 0
    for code in codes:
        target = flatten_file(UI_LANGS_DIR / f"{code}.json")
        faltando = missing_keys(base, target)
        extras = extra_keys(base, target)
        total_faltando += len(faltando)
        status = "ok" if not faltando else f"faltando {len(faltando)}"
        print(f"  {code:<6} {len(target):>4} chaves  |  {status}")
        for key in faltando[:10]:
            print(f"        - {key}")
        if len(faltando) > 10:
            print(f"        ... e mais {len(faltando) - 10}")
        for key in extras[:10]:
            print(f"        + {key} (não existe na referência)")

    if total_faltando:
        print(f"\n{total_faltando} chave(s) pendente(s). Use --translate para completar.")
        return 1
    print("\nTodos os idiomas estão com a mesma paridade de chaves.")
    return 0


def run_translate(codes: list[str], model: str, api_key: str, dry_run: bool) -> int:
    base = flatten_file(UI_LANGS_DIR / f"{BASE_LANGUAGE}.json")
    pendentes_total = 0

    for code in codes:
        path = UI_LANGS_DIR / f"{code}.json"
        target = flatten_file(path)
        pendentes = missing_keys(base, target)
        if not pendentes:
            print(f"  [{code}] nada a traduzir.")
            continue

        print(f"  [{code}] {len(pendentes)} chave(s) para traduzir...")
        if dry_run:
            payload = build_payload(pendentes, base, code, model)
            print(json.dumps(payload, ensure_ascii=False, indent=2)[:2000])
            pendentes_total += len(pendentes)
            continue

        traducao = read_json(path)
        for lote in chunks(pendentes, BATCH_SIZE):
            aprovadas = translate_batch(lote, base, code, model, api_key)
            for key, value in aprovadas.items():
                set_nested(traducao, key, value)
            print(f"  [{code}] lote de {len(lote)} chave(s): {len(aprovadas)} aceita(s).")

        if not dry_run:
            write_json(path, traducao)
            print(f"  [{code}] gravado em langs/ui_langs/{code}.json.")
        pendentes_total += len(pendentes)

    if dry_run:
        print(f"\n[dry-run] {pendentes_total} chave(s) seriam enviadas; nenhum arquivo foi alterado.")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Confere e completa as traduções da interface do DesktopORZ.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="apenas confere a paridade de chaves (padrão).",
    )
    parser.add_argument(
        "--translate",
        action="store_true",
        help="pede as traduções que faltam ao gateway da Respan e grava os arquivos.",
    )
    parser.add_argument(
        "--lang",
        default="",
        help="siglas separadas por vírgula (ex: es,zh). Padrão: todos os idiomas.",
    )
    parser.add_argument(
        "--model",
        default="",
        help=f"modelo usado no gateway (padrão: {MODEL_VAR} ou {DEFAULT_MODEL}).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="monta o pedido do gateway e mostra o conteúdo, sem enviar nem gravar.",
    )
    args = parser.parse_args(argv)

    if not UI_LANGS_DIR.is_dir():
        print(f"Pasta não encontrada: {UI_LANGS_DIR}", file=sys.stderr)
        return 1

    load_env_file()
    api_key, model_do_ambiente = gateway_credentials()
    model = args.model.strip() or model_do_ambiente

    disponiveis = language_codes()
    if args.lang.strip():
        pedidos = [code.strip() for code in args.lang.split(",") if code.strip()]
        desconhecidos = [code for code in pedidos if code not in disponiveis]
        if desconhecidos:
            print(f"Idioma(s) sem arquivo: {', '.join(desconhecidos)}", file=sys.stderr)
            return 1
        codes = pedidos
    else:
        codes = disponiveis

    if not args.translate:
        return run_check(codes)

    if not api_key and not args.dry_run:
        print(
            f"{API_KEY_VAR} não está definida. Configure-a no ambiente ou no `.env` "
            "(veja `.env.example`) para falar com o gateway da Respan.",
            file=sys.stderr,
        )
        return 2

    print(f"Gateway Respan: {GATEWAY_URL} (modelo {model})")
    try:
        return run_translate(codes, model, api_key, args.dry_run)
    except urllib.error.HTTPError as error:
        detalhe = error.read().decode("utf-8", "replace")[:400]
        print(f"Falha no gateway ({error.code}): {detalhe}", file=sys.stderr)
        if error.code in (401, 403):
            print("Confira a chave e se a conta da Respan está ativa.", file=sys.stderr)
        return 1
    except urllib.error.URLError as error:
        print(f"Não foi possível falar com o gateway: {error.reason}", file=sys.stderr)
        return 1
    except ValueError as error:
        print(f"Resposta inválida do gateway: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())