#!/usr/bin/env python3
"""Verify Unica XML against the approved 1C 8.3.27 evidence profile."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

from lxml import etree


XS_NS = "http://www.w3.org/2001/XMLSchema"
XSI_NS = "http://www.w3.org/2001/XMLSchema-instance"


class SourceError(RuntimeError):
    """Approved schema evidence is absent, altered, ambiguous, or unusable."""


class CorpusError(RuntimeError):
    """The corpus manifest cannot be trusted or completely processed."""


class RuntimeEvidence(dict):
    def close(self):
        temporary = self.pop("_temporaryDirectory", None)
        if temporary is not None:
            temporary.cleanup()

    def __del__(self):
        self.close()


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _safe_zip_members(archive: zipfile.ZipFile) -> list[zipfile.ZipInfo]:
    infos = archive.infolist()
    names = [info.filename for info in infos]
    if len(names) != len(set(names)):
        raise SourceError("duplicate ZIP member")
    for info in infos:
        path = PurePosixPath(info.filename)
        if path.is_absolute() or ".." in path.parts or not path.parts:
            raise SourceError(f"unsafe ZIP path: {info.filename}")
        mode = info.external_attr >> 16
        if stat.S_ISLNK(mode):
            raise SourceError(f"ZIP symlink is forbidden: {info.filename}")
    return infos


def _manifest_member(infos: list[zipfile.ZipInfo]) -> str:
    matches = [info.filename for info in infos if PurePosixPath(info.filename).name == "manifest.json"]
    if len(matches) != 1:
        raise SourceError(f"runtime archive must contain exactly one manifest.json, found {len(matches)}")
    return matches[0]


def _schema_target_namespace(payload: bytes, label: str) -> str:
    try:
        root = etree.fromstring(payload, parser=etree.XMLParser(resolve_entities=False, no_network=True))
    except etree.XMLSyntaxError as error:
        raise SourceError(f"invalid XSD {label}: {error}") from error
    if root.tag != f"{{{XS_NS}}}schema":
        raise SourceError(f"XSD root is not xs:schema: {label}")
    namespace = root.get("targetNamespace")
    if not namespace:
        raise SourceError(f"XSD has no targetNamespace: {label}")
    return namespace


def _expected_runtime(profile: dict) -> dict:
    runtime = profile.get("runtime")
    if not isinstance(runtime, dict):
        raise SourceError("profile has no runtime contract")
    return runtime


def verified_runtime(path: Path | str, profile: dict) -> dict:
    path = Path(path)
    expected = _expected_runtime(profile)
    if not path.is_file():
        raise SourceError(f"runtime XSD archive is missing: {path}")
    actual_hash = file_sha256(path)
    if actual_hash != expected.get("sha256"):
        raise SourceError(f"runtime archive SHA-256 mismatch: {actual_hash}")

    temporary = tempfile.TemporaryDirectory(prefix="unica-8-3-27-xsd-")
    temp_root = Path(temporary.name)
    try:
        with zipfile.ZipFile(path) as archive:
            infos = _safe_zip_members(archive)
            manifest_name = _manifest_member(infos)
            try:
                manifest = json.loads(archive.read(manifest_name))
            except (json.JSONDecodeError, UnicodeDecodeError, KeyError) as error:
                raise SourceError(f"invalid runtime manifest: {error}") from error
            if manifest.get("formatVersion") != expected.get("formatVersion"):
                raise SourceError("runtime manifest formatVersion mismatch")
            if manifest.get("platformVersion") != expected.get("platformVersion"):
                raise SourceError("runtime manifest platformVersion mismatch")
            if manifest.get("summary") != expected.get("summary"):
                raise SourceError("runtime manifest summary mismatch")
            declarations = manifest.get("schemas")
            if not isinstance(declarations, list) or not declarations:
                raise SourceError("runtime manifest schemas are missing or empty")
            prefix = str(PurePosixPath(manifest_name).parent)
            prefix = "" if prefix == "." else prefix + "/"
            declared_names = set()
            namespace_paths = {}
            schema_rows = []
            for declaration in declarations:
                relative = declaration.get("file")
                if not isinstance(relative, str):
                    raise SourceError("runtime schema declaration has no file")
                pure = PurePosixPath(relative)
                if pure.is_absolute() or ".." in pure.parts:
                    raise SourceError(f"unsafe manifest schema path: {relative}")
                member = prefix + relative
                if member in declared_names:
                    raise SourceError(f"duplicate runtime schema declaration: {relative}")
                declared_names.add(member)
                try:
                    payload = archive.read(member)
                except KeyError as error:
                    raise SourceError(f"declared XSD is missing: {relative}") from error
                if len(payload) != declaration.get("size"):
                    raise SourceError(f"manifest size mismatch: {relative}")
                if hashlib.sha256(payload).hexdigest() != declaration.get("sha256"):
                    raise SourceError(f"manifest hash mismatch: {relative}")
                namespace = _schema_target_namespace(payload, relative)
                if namespace != declaration.get("targetNamespace"):
                    raise SourceError(f"manifest targetNamespace mismatch: {relative}")
                if namespace in namespace_paths:
                    raise SourceError(f"duplicate targetNamespace: {namespace}")
                target = temp_root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(payload)
                namespace_paths[namespace] = target
                schema_rows.append({"file": relative, "targetNamespace": namespace})
            exports = manifest.get("exports")
            if not isinstance(exports, list) or len(exports) != manifest["summary"]["packages"]:
                raise SourceError("runtime manifest exports/package count mismatch")
            exported_files = set()
            exported_namespaces = set()
            for export in exports:
                namespace = export.get("sourceNamespace")
                if not isinstance(namespace, str) or namespace in exported_namespaces:
                    raise SourceError(f"duplicate or invalid exported namespace: {namespace}")
                exported_namespaces.add(namespace)
                if export.get("error") not in (None, ""):
                    raise SourceError(f"runtime export reports an error for {namespace}")
                for relative in export.get("files", []):
                    exported_files.add(prefix + relative)
            if exported_files != declared_names or exported_namespaces != set(namespace_paths):
                raise SourceError("runtime exports do not exactly match declared schemas/namespaces")
            archived_xsd = {info.filename for info in infos if info.filename.lower().endswith(".xsd")}
            undeclared = sorted(archived_xsd - declared_names)
            if undeclared:
                raise SourceError(f"undeclared XSD in runtime archive: {undeclared[0]}")

        _resolve_imports(namespace_paths)
        matrix = _compile_matrix(schema_rows, temp_root, expected.get("knownCompileFailures", {}))
        strict_namespaces = {
            family.get("schemaNamespace")
            for family in profile.get("families", {}).values()
            if family.get("coverage") == "strict"
        }
        failed = {row["targetNamespace"]: row for row in matrix if row["status"] != "compiled"}
        for namespace in strict_namespaces:
            if namespace not in namespace_paths:
                raise SourceError(f"strict schema namespace is missing: {namespace}")
            if namespace in failed:
                raise SourceError(f"strict schema dependency failed to compile: {namespace}: {failed[namespace]['detail']}")
        wrappers = {}
        for family in profile.get("families", {}).values():
            if family.get("coverage") != "strict":
                continue
            namespace = family["schemaNamespace"]
            wrappers[(namespace, family["wrapperType"])] = _compile_wrapper(
                namespace_paths[namespace], namespace, family["wrapperType"], temp_root
            )
        return RuntimeEvidence({
            "source": {
                "kind": "runtime-xsd",
                "sha256": actual_hash,
                "formatVersion": manifest["formatVersion"],
                "platformVersion": manifest["platformVersion"],
                "identityStatement": "Configured SHA-256 proves identity with approved local evidence, not download provenance.",
            },
            "compilationMatrix": matrix,
            "namespacePaths": namespace_paths,
            "wrappers": wrappers,
            "_temporaryDirectory": temporary,
        })
    except Exception:
        temporary.cleanup()
        raise


def _resolve_imports(namespace_paths: dict[str, Path]) -> None:
    for path in namespace_paths.values():
        tree = etree.parse(str(path), parser=etree.XMLParser(resolve_entities=False, no_network=True))
        changed = False
        for node in tree.findall(f".//{{{XS_NS}}}import"):
            if node.get("schemaLocation"):
                continue
            namespace = node.get("namespace")
            target = namespace_paths.get(namespace)
            if target is None:
                raise SourceError(f"unresolved namespace import {namespace!r} in {path.name}")
            node.set("schemaLocation", os.path.relpath(target, path.parent))
            changed = True
        if changed:
            tree.write(str(path), encoding="UTF-8", xml_declaration=True)


def _compile_matrix(rows: list[dict], root: Path, known: dict) -> list[dict]:
    matrix = []
    unexpected = []
    for row in sorted(rows, key=lambda item: item["file"]):
        path = root / row["file"]
        result = dict(row)
        try:
            etree.XMLSchema(etree.parse(str(path), parser=etree.XMLParser(no_network=True, resolve_entities=False)))
            result.update(status="compiled", detail="")
        except (etree.XMLSchemaParseError, etree.XMLSyntaxError) as error:
            reason = known.get(PurePosixPath(row["file"]).name) or known.get(row["file"])
            if reason:
                result.update(status="known-source-incompatibility", detail=reason)
            else:
                detail = str(error.error_log.last_error or error).replace(str(root), "<runtime-xsd>")
                result.update(status="compile-error", detail=detail)
                unexpected.append(result)
        matrix.append(result)
    if unexpected:
        first = unexpected[0]
        raise SourceError(f"unexpected XSD compile failure {first['file']}: {first['detail']}")
    return matrix


def _compile_wrapper(schema_path: Path, namespace: str, type_name: str, root: Path):
    wrapper = root / f"wrapper-{hashlib.sha256((namespace + type_name).encode()).hexdigest()[:12]}.xsd"
    include = os.path.relpath(schema_path, wrapper.parent)
    content = f'''<xs:schema xmlns:xs="{XS_NS}" xmlns:tns="{namespace}" targetNamespace="{namespace}" elementFormDefault="qualified">
<xs:include schemaLocation="{include}"/>
<xs:element name="{type_name}" type="tns:{type_name}"/>
</xs:schema>'''
    wrapper.write_text(content, encoding="utf-8")
    try:
        return etree.XMLSchema(etree.parse(str(wrapper), parser=etree.XMLParser(no_network=True, resolve_entities=False)))
    except etree.XMLSchemaParseError as error:
        raise SourceError(f"strict schema wrapper failed for {namespace} {type_name}: {error}") from error


def strict_xsd_error(runtime: dict, namespace: str, type_name: str, xml_text: str) -> str | None:
    try:
        document = etree.fromstring(xml_text.encode(), parser=etree.XMLParser(no_network=True, resolve_entities=False))
    except etree.XMLSyntaxError as error:
        return str(error)
    schema = runtime["wrappers"][(namespace, type_name)]
    if schema.validate(document):
        return None
    return str(schema.error_log.last_error or "XSD validation failed")


def _safe_corpus_path(root: Path, raw: str) -> Path:
    pure = PurePosixPath(raw)
    if pure.is_absolute() or ".." in pure.parts:
        raise CorpusError(f"unsafe corpus path: {raw}")
    candidate = root
    for part in pure.parts:
        candidate = candidate / part
        if candidate.is_symlink():
            raise CorpusError(f"corpus symlink is forbidden: {raw}")
    resolved_root = root.resolve()
    try:
        candidate.resolve().relative_to(resolved_root)
    except ValueError as error:
        raise CorpusError(f"corpus path escapes root: {raw}") from error
    return candidate


def _load_corpus(manifest_path: Path, expected_profile: str) -> tuple[dict, Path, list[dict]]:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CorpusError(f"invalid corpus manifest: {error}") from error
    if manifest.get("schemaVersion") != 1 or manifest.get("profile") != expected_profile:
        raise CorpusError("corpus schemaVersion/profile mismatch")
    root = manifest_path.parent
    files = []
    seen = set()
    for case in manifest.get("cases", []):
        if not isinstance(case.get("files"), list):
            raise CorpusError("corpus case has no files list")
        for entry in case["files"]:
            raw = entry.get("path")
            if not isinstance(raw, str) or raw in seen:
                raise CorpusError(f"duplicate or missing corpus file: {raw}")
            seen.add(raw)
            path = _safe_corpus_path(root, raw)
            if not path.is_file():
                raise CorpusError(f"listed corpus file is missing: {raw}")
            if file_sha256(path) != entry.get("sha256"):
                raise CorpusError(f"corpus hash mismatch: {raw}")
            if not isinstance(entry.get("family"), str):
                raise CorpusError(f"corpus file has no family: {raw}")
            if not isinstance(entry.get("seed"), bool):
                raise CorpusError(f"corpus file must explicitly declare seed: {raw}")
            files.append({**entry, "caseId": case.get("id"), "toolId": case.get("toolId"), "xmlImpact": case.get("xmlImpact"), "_path": path})
    if not files:
        raise CorpusError("empty or unprocessed corpus")
    for path in root.rglob("*"):
        if path.is_symlink():
            raise CorpusError(f"corpus symlink is forbidden: {path.relative_to(root)}")
    actual_xml = {path.relative_to(root).as_posix() for path in root.rglob("*.xml") if path.is_file()}
    unlisted = sorted(actual_xml - seen)
    if unlisted:
        raise CorpusError(f"unlisted XML in corpus: {unlisted[0]}")
    return manifest, root, files


def _qname(root) -> str:
    return str(etree.QName(root))


def _unresolved_qname_prefix(root) -> str | None:
    for node in root.iter():
        for name, value in node.attrib.items():
            if name == f"{{{XSI_NS}}}type" and ":" in value:
                prefix = value.split(":", 1)[0]
                if prefix not in node.nsmap:
                    return prefix
        if etree.QName(node).localname in {"Type", "ValueType"}:
            value = (node.text or "").strip()
            if ":" in value:
                prefix = value.split(":", 1)[0]
                if prefix not in node.nsmap:
                    return prefix
    return None


def verify_corpus(manifest_path: Path | str, profile: dict, runtime: dict, edt: dict | None) -> tuple[dict, int]:
    _, root, entries = _load_corpus(Path(manifest_path), profile["profile"])
    listed = {entry["path"]: entry for entry in entries}
    families_by_id = {}
    for expected_qname, configured in profile.get("families", {}).items():
        family_id = configured.get("id")
        if family_id in families_by_id:
            raise SourceError(f"duplicate configured family id: {family_id}")
        families_by_id[family_id] = (expected_qname, configured)
    rows = []
    violation = False
    inconclusive = False
    for entry in sorted(entries, key=lambda item: item["path"]):
        checks = []
        try:
            xml_root = etree.fromstring(entry["_path"].read_bytes(), parser=etree.XMLParser(no_network=True, resolve_entities=False))
            checks.append({"name": "wellFormed", "status": "pass"})
        except etree.XMLSyntaxError as error:
            rows.append(_file_row(entry, None, "unknown", [{"name": "wellFormed", "status": "fail", "detail": str(error)}], "fail", "repair XML syntax"))
            violation = True
            continue
        qname = _qname(xml_root)
        selected = families_by_id.get(entry.get("family"))
        if selected is None:
            raise CorpusError(f"unknown corpus family: {entry.get('family')}")
        expected_qname, family = selected
        root_status = "pass" if qname == expected_qname else "fail"
        checks.append({"name": "rootQName", "status": root_status, "actual": qname, "expected": expected_qname})
        violation |= root_status == "fail"
        prefix = _unresolved_qname_prefix(xml_root)
        if prefix:
            checks.append({"name": "qnamePrefixes", "status": "fail", "detail": f"unresolved prefix: {prefix}"})
            violation = True
        else:
            checks.append({"name": "qnamePrefixes", "status": "pass"})
        version_mode = family.get("version", "none")
        if version_mode == "root":
            actual = xml_root.get("version")
            status = "pass" if actual == profile["exportVersion"] else "fail"
            checks.append({"name": "exportVersion", "status": status, "actual": actual, "expected": profile["exportVersion"]})
            violation |= status == "fail"
        elif version_mode == "owner":
            owner_path = entry.get("ownerPath")
            if owner_path:
                owner = listed.get(owner_path)
                if owner is None:
                    raise CorpusError(f"ownerPath is not a listed corpus file: {owner_path}")
                owner_selected = families_by_id.get(owner.get("family"))
                if owner_selected is None or owner_selected[1].get("version") != "root":
                    raise CorpusError(f"ownerPath is not a listed version-bearing descriptor: {owner_path}")
                try:
                    owner_root = etree.fromstring(owner["_path"].read_bytes(), parser=etree.XMLParser(no_network=True, resolve_entities=False))
                    actual = owner_root.get("version")
                except etree.XMLSyntaxError as error:
                    actual = None
                    checks.append({"name": "ownerVersion", "status": "fail", "detail": str(error)})
                else:
                    status = "pass" if actual == profile["exportVersion"] else "fail"
                    checks.append({"name": "ownerVersion", "status": status, "actual": actual, "expected": profile["exportVersion"], "ownerPath": owner_path})
                    violation |= status == "fail"
            elif entry.get("newStandalone") is True and entry.get("xmlImpact") == "created" and entry.get("seed") is False:
                checks.append({"name": "ownerVersion", "status": "pass", "detail": "explicit new standalone content"})
            else:
                checks.append({"name": "ownerVersion", "status": "fail", "detail": "ownerPath or newStandalone=true is required"})
                violation = True

        coverage = family["coverage"]
        xsd_error = None
        if coverage == "strict":
            xsd_error = strict_xsd_error(runtime, family["schemaNamespace"], family["wrapperType"], etree.tostring(xml_root, encoding="unicode"))
            checks.append({"name": "runtimeXsd", "status": "fail" if xsd_error else "pass", "detail": xsd_error or ""})
            violation |= xsd_error is not None
            result = "fail" if any(check["status"] == "fail" for check in checks) else "pass"
            next_evidence = "none" if result == "pass" else "fix document contract or strict XSD violation"
        else:
            if coverage == "not-covered" and family.get("edtDeclaration"):
                declaration = bool(edt and edt.get("declarations", {}).get(family["edtDeclaration"]))
                checks.append({"name": "edtDeclaration", "status": "pass" if declaration else "unavailable"})
            result = "fail" if any(check["status"] == "fail" for check in checks) else "inconclusive"
            next_evidence = "platform 8.3.27.2074 load/dump roundtrip"
            inconclusive |= result == "inconclusive"
        rows.append(_file_row(entry, qname, coverage, checks, result, next_evidence))
    exit_code = 1 if violation or any(row["result"] == "fail" for row in rows) else (3 if inconclusive else 0)
    verdict = {0: "pass", 1: "fail", 3: "inconclusive"}[exit_code]
    report = {
        "schemaVersion": 1,
        "profile": profile["profile"],
        "verdict": verdict,
        "exitCode": exit_code,
        "sources": {"runtime": runtime["source"], "edt": _public_edt(edt)},
        "schemaCompilation": runtime["compilationMatrix"],
        "files": rows,
        "summary": {
            "files": len(rows),
            "passed": sum(row["result"] == "pass" for row in rows),
            "failed": sum(row["result"] == "fail" for row in rows),
            "inconclusive": sum(row["result"] == "inconclusive" for row in rows),
        },
    }
    return report, exit_code


def _file_row(entry, qname, coverage, checks, result, next_evidence):
    return {
        "caseId": entry.get("caseId"), "toolId": entry.get("toolId"),
        "xmlImpact": entry.get("xmlImpact"), "path": entry.get("path"),
        "sha256": entry.get("sha256"), "rootQName": qname,
        "coverage": coverage, "checks": checks, "result": result,
        "requiredNextEvidence": next_evidence,
    }


def _public_edt(edt):
    if not edt:
        return {"provided": False, "evidenceScope": "none"}
    return {key: value for key, value in edt.items() if not key.startswith("_")}


def _unfold_manifest(text: str) -> dict[str, str]:
    logical = []
    for line in text.replace("\r\n", "\n").split("\n"):
        if line.startswith(" ") and logical:
            logical[-1] += line[1:]
        else:
            logical.append(line)
    result = {}
    for line in logical:
        if ": " in line:
            key, value = line.split(": ", 1)
            result[key] = value
    return result


def verified_edt(path: Path | str, profile: dict, runner=subprocess.run) -> dict:
    path = Path(path)
    expected = profile.get("edt", {})
    if not path.is_file() or file_sha256(path) != expected.get("sha256"):
        raise SourceError("EDT jar SHA-256 mismatch or file missing")
    completed = runner(["jarsigner", "-verify", "-certs", str(path)], capture_output=True, text=True)
    if completed.returncode != 0 or "jar verified" not in (completed.stdout + completed.stderr).lower():
        raise SourceError("jarsigner verification failed for EDT jar")
    with zipfile.ZipFile(path) as archive:
        infos = _safe_zip_members(archive)
        names = {info.filename for info in infos}
        try:
            headers = _unfold_manifest(archive.read("META-INF/MANIFEST.MF").decode("utf-8"))
        except (KeyError, UnicodeDecodeError) as error:
            raise SourceError(f"invalid EDT OSGi manifest: {error}") from error
        if headers.get("Bundle-SymbolicName", "").split(";", 1)[0] != expected.get("symbolicName"):
            raise SourceError("EDT Bundle-SymbolicName mismatch")
        if headers.get("Bundle-Version") != expected.get("version"):
            raise SourceError("EDT Bundle-Version mismatch")
        entry_text = {}
        for label, name in expected.get("entries", {}).items():
            if name not in names:
                raise SourceError(f"required EDT XDTO entry is missing: {name}")
            entry_text[label] = archive.read(name).decode("utf-8")
        declarations = {}
        for label, rule in expected.get("declarations", {}).items():
            text = entry_text.get(rule["entry"], "")
            declarations[label] = all(token in text for token in rule.get("tokens", []))
            if not declarations[label]:
                raise SourceError(f"EDT declaration evidence is missing: {label}")
    return {
        "provided": True, "sha256": expected["sha256"],
        "bundleSymbolicName": expected["symbolicName"], "bundleVersion": expected["version"],
        "evidenceScope": "8.3.27-line declaration evidence; not proof of patch build 8.3.27.2074",
        "identityStatement": "Configured SHA-256 proves identity with approved local evidence, not download provenance.",
        "declarations": declarations,
    }


def report_matches_schema(value, schema) -> bool:
    if "const" in schema and value != schema["const"]:
        return False
    if "oneOf" in schema and sum(report_matches_schema(value, choice) for choice in schema["oneOf"]) != 1:
        return False
    if "enum" in schema and value not in schema["enum"]:
        return False
    expected_type = schema.get("type")
    if expected_type == "object":
        if not isinstance(value, dict):
            return False
        if any(key not in value for key in schema.get("required", [])):
            return False
        properties = schema.get("properties", {})
        return all(key not in value or report_matches_schema(value[key], child) for key, child in properties.items())
    if expected_type == "array":
        return isinstance(value, list) and all(report_matches_schema(item, schema.get("items", {})) for item in value)
    if expected_type == "string":
        return isinstance(value, str)
    if expected_type == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected_type == "boolean":
        return isinstance(value, bool)
    return True


def _error_report(profile_id: str, error: Exception) -> dict:
    return {"schemaVersion": 1, "profile": profile_id, "verdict": "source-error", "exitCode": 2, "sourceError": str(error), "sources": {}, "schemaCompilation": [], "files": [], "summary": {"files": 0, "passed": 0, "failed": 0, "inconclusive": 0}}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-xsd-zip", required=True, type=Path)
    parser.add_argument("--edt-xdto-jar", required=True, type=Path)
    parser.add_argument("--corpus", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--profile", type=Path, default=Path(__file__).with_name("verify-8-3-27-xml-profile.json"), help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    runtime = None
    try:
        runtime = verified_runtime(args.runtime_xsd_zip, profile)
        edt = verified_edt(args.edt_xdto_jar, profile)
        report, status = verify_corpus(args.corpus, profile, runtime, edt)
    except (SourceError, CorpusError) as error:
        report, status = _error_report(profile.get("profile", "unknown"), error), 2
    finally:
        if runtime is not None:
            runtime.close()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return status


if __name__ == "__main__":
    sys.exit(main())
