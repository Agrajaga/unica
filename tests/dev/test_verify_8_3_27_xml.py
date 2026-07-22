import hashlib
import importlib.util
import json
import stat
import subprocess
import tempfile
import unittest
import warnings
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/dev/verify-8-3-27-xml.py"
PROFILE = ROOT / "scripts/dev/verify-8-3-27-xml-profile.json"
REPORT_SCHEMA = ROOT / "scripts/dev/verify-8-3-27-xml-report.schema.json"


def load_verifier():
    spec = importlib.util.spec_from_file_location("verify_8_3_27_xml", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


XS = "http://www.w3.org/2001/XMLSchema"
DCS_NS = "urn:test:dcs"
ROLE_NS = "urn:test:roles"
MXL_NS = "urn:test:mxl"
MD_NS = "urn:test:md"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def schema(namespace, body, imports=""):
    return (
        f'<xs:schema xmlns:xs="{XS}" xmlns:tns="{namespace}" '
        f'targetNamespace="{namespace}" elementFormDefault="qualified">'
        f"{imports}{body}</xs:schema>"
    ).encode()


def runtime_archive(root, schemas, *, extra=None, summary=None):
    entries = []
    exports = []
    for index, (namespace, payload) in enumerate(schemas, 1):
        name = f"schemas/{index:04}.xsd"
        entries.append({"file": name, "targetNamespace": namespace, "sha256": sha(payload), "size": len(payload)})
        exports.append({"sourceNamespace": namespace, "files": [name], "error": ""})
    count = len(entries)
    manifest = {
        "formatVersion": 1,
        "platformVersion": "test-platform",
        "summary": summary or {"packages": count, "namespaces": count, "success": count, "schemas": count, "errors": 0},
        "exports": exports,
        "schemas": entries,
    }
    target = root / "runtime.zip"
    with zipfile.ZipFile(target, "w") as archive:
        archive.writestr("export/manifest.json", json.dumps(manifest))
        for entry, (_, payload) in zip(entries, schemas):
            archive.writestr("export/" + entry["file"], payload)
        for name, payload, attrs in extra or []:
            info = zipfile.ZipInfo(name)
            info.external_attr = attrs
            archive.writestr(info, payload)
    return target


def test_profile(runtime_zip):
    return {
        "profile": "test-2.20",
        "exportVersion": "2.20",
        "runtime": {
            "sha256": sha(runtime_zip.read_bytes()),
            "formatVersion": 1,
            "platformVersion": "test-platform",
            "summary": {"packages": 2, "namespaces": 2, "success": 2, "schemas": 2, "errors": 0},
            "knownCompileFailures": {},
        },
        "edt": {"sha256": "unused", "symbolicName": "test", "version": "1", "entries": {}, "declarations": {}},
        "families": {
            f"{{{DCS_NS}}}DataCompositionSchema": {
                "id": "dcs", "coverage": "strict", "schemaNamespace": DCS_NS,
                "wrapperType": "DataCompositionSchema", "wrapper": "dcs-root-alias",
                "version": "owner"
            },
            f"{{{ROLE_NS}}}Rights": {
                "id": "roles", "coverage": "strict", "schemaNamespace": ROLE_NS,
                "wrapperType": "Rights", "wrapper": "roles-global-element", "version": "root"
            },
            f"{{{MXL_NS}}}document": {
                "id": "mxl", "coverage": "known-schema-incompatibility", "version": "owner"
            },
            f"{{{MD_NS}}}MetaDataObject": {
                "id": "metadata", "coverage": "not-covered", "version": "root",
                "edtDeclaration": "metadata"
            }
        }
    }


def rewrite_manifest(source, target, mutate):
    with zipfile.ZipFile(source) as archive:
        members = [(info.filename, archive.read(info.filename)) for info in archive.infolist()]
    with zipfile.ZipFile(target, "w") as archive:
        for name, payload in members:
            if name.endswith("manifest.json"):
                value = json.loads(payload)
                mutate(value)
                payload = json.dumps(value).encode()
            archive.writestr(name, payload)
    return target


def basic_schemas():
    dcs = schema(DCS_NS, '<xs:complexType name="DataCompositionSchema"><xs:sequence><xs:element name="name" type="xs:string"/></xs:sequence></xs:complexType>')
    roles = schema(ROLE_NS, '<xs:complexType name="Rights"><xs:sequence><xs:element name="setForNewObjects" type="xs:boolean"/></xs:sequence><xs:attribute name="version" type="xs:string" use="required"/></xs:complexType>')
    return [(DCS_NS, dcs), (ROLE_NS, roles)]


def write_corpus(root, files):
    cases = []
    listed = []
    case_dir = root / "case"
    case_dir.mkdir(parents=True)
    for name, text, extra in files:
        path = case_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        parsed = __import__("lxml.etree", fromlist=["etree"]).fromstring(text.encode())
        qname = str(__import__("lxml.etree", fromlist=["etree"]).QName(parsed))
        family = {
            f"{{{DCS_NS}}}DataCompositionSchema": "dcs",
            f"{{{ROLE_NS}}}Rights": "roles",
            f"{{{MXL_NS}}}document": "mxl",
            f"{{{MD_NS}}}MetaDataObject": "metadata",
        }.get(qname, "unknown")
        item = {"path": path.relative_to(root).as_posix(), "sha256": sha(path.read_bytes()), "seed": False, "family": family}
        item.update(extra)
        listed.append(item)
    cases.append({"id": "case", "toolId": "unica.test", "xmlImpact": "created", "files": listed})
    manifest = root / "corpus-manifest.json"
    manifest.write_text(json.dumps({"schemaVersion": 1, "profile": "test-2.20", "cases": cases}), encoding="utf-8")
    return manifest


def edt_jar(root):
    target = root / "edt.jar"
    manifest = "Manifest-Version: 1.0\r\nBundle-SymbolicName: test\r\nBundle-Version: 1\r\n\r\n"
    with zipfile.ZipFile(target, "w") as archive:
        archive.writestr("META-INF/MANIFEST.MF", manifest)
        archive.writestr("backend.xdto", 'targetNamespace="urn:test:md" name="MetaDataObject"')
    return target


class VerifierContractTests(unittest.TestCase):
    def test_checked_in_verifier_contract_files_exist(self):
        self.assertTrue(SCRIPT.is_file())
        self.assertTrue(PROFILE.is_file())
        self.assertTrue(REPORT_SCHEMA.is_file())

    def test_runtime_rejects_hash_mismatch_and_zip_traversal(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            profile["runtime"]["sha256"] = "0" * 64
            with self.assertRaisesRegex(verifier.SourceError, "SHA-256"):
                verifier.verified_runtime(archive, profile)
            traversal = runtime_archive(root, basic_schemas(), extra=[("../escape.xsd", b"x", 0)])
            profile = test_profile(traversal)
            with self.assertRaisesRegex(verifier.SourceError, "unsafe ZIP"):
                verifier.verified_runtime(traversal, profile)

    def test_runtime_rejects_symlink_and_manifest_hash_or_namespace_mismatch(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            attrs = (stat.S_IFLNK | 0o777) << 16
            archive = runtime_archive(root, basic_schemas(), extra=[("export/link", b"target", attrs)])
            with self.assertRaisesRegex(verifier.SourceError, "symlink"):
                verifier.verified_runtime(archive, test_profile(archive))

            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            duplicate = root / "duplicate.zip"
            duplicate.write_bytes(archive.read_bytes())
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                with zipfile.ZipFile(duplicate, "a") as out:
                    out.writestr("export/schemas/0001.xsd", b"changed")
            profile["runtime"]["sha256"] = sha(duplicate.read_bytes())
            with self.assertRaisesRegex(verifier.SourceError, "duplicate ZIP member"):
                verifier.verified_runtime(duplicate, profile)

            bad_hash = rewrite_manifest(archive, root / "bad-hash.zip", lambda value: value["schemas"][0].update(sha256="0" * 64))
            profile = test_profile(bad_hash)
            with self.assertRaisesRegex(verifier.SourceError, "manifest hash mismatch"):
                verifier.verified_runtime(bad_hash, profile)

            bad_namespace = rewrite_manifest(archive, root / "bad-namespace.zip", lambda value: value["schemas"][0].update(targetNamespace="urn:wrong"))
            profile = test_profile(bad_namespace)
            with self.assertRaisesRegex(verifier.SourceError, "manifest targetNamespace mismatch"):
                verifier.verified_runtime(bad_namespace, profile)

    def test_import_resolution_duplicate_namespace_and_strict_compile_failure(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            imported = schema("urn:test:base", '<xs:simpleType name="Name"><xs:restriction base="xs:string"/></xs:simpleType>')
            imports = '<xs:import namespace="urn:test:base"/>'
            consumer = schema(DCS_NS, '<xs:complexType name="DataCompositionSchema"><xs:sequence><xs:element name="name" type="b:Name" xmlns:b="urn:test:base"/></xs:sequence></xs:complexType>', imports)
            archive = runtime_archive(root, [(DCS_NS, consumer), ("urn:test:base", imported)])
            profile = test_profile(archive)
            profile["families"].pop(f"{{{ROLE_NS}}}Rights")
            verified = verifier.verified_runtime(archive, profile)
            self.assertTrue(all(row["status"] == "compiled" for row in verified["compilationMatrix"]))
            verified.close()

            duplicate = runtime_archive(root, [(DCS_NS, consumer), (DCS_NS, consumer)])
            profile = test_profile(duplicate)
            with self.assertRaisesRegex(verifier.SourceError, "duplicate targetNamespace"):
                verifier.verified_runtime(duplicate, profile)

            broken = schema(DCS_NS, '<xs:complexType name="DataCompositionSchema"><xs:restriction/></xs:complexType>')
            archive = runtime_archive(root, [(DCS_NS, broken), (ROLE_NS, basic_schemas()[1][1])])
            profile = test_profile(archive)
            with self.assertRaisesRegex(verifier.SourceError, "XSD compile failure|strict schema"):
                verifier.verified_runtime(archive, profile)

            unresolved = schema(DCS_NS, '<xs:complexType name="DataCompositionSchema"/>', '<xs:import namespace="urn:missing"/>')
            archive = runtime_archive(root, [(DCS_NS, unresolved), (ROLE_NS, basic_schemas()[1][1])])
            profile = test_profile(archive)
            with self.assertRaisesRegex(verifier.SourceError, "unresolved namespace import"):
                verifier.verified_runtime(archive, profile)

    def test_dcs_and_rights_wrappers_preserve_schema_content(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            runtime = verifier.verified_runtime(archive, test_profile(archive))
            self.assertIsNone(verifier.strict_xsd_error(runtime, DCS_NS, "DataCompositionSchema", "<DataCompositionSchema xmlns='urn:test:dcs'><name>x</name></DataCompositionSchema>"))
            self.assertIn("name", verifier.strict_xsd_error(runtime, DCS_NS, "DataCompositionSchema", "<DataCompositionSchema xmlns='urn:test:dcs'/>"))
            self.assertIn("setForNewObjects", verifier.strict_xsd_error(runtime, ROLE_NS, "Rights", "<Rights xmlns='urn:test:roles' version='2.20'/>"))

    def test_corpus_contract_and_all_exit_classes(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            runtime = verifier.verified_runtime(archive, profile)

            corpus = write_corpus(root / "strict", [("rights.xml", "<Rights xmlns='urn:test:roles' version='2.20'><setForNewObjects>true</setForNewObjects></Rights>", {})])
            report, status = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(status, 0)
            self.assertEqual((report["verdict"], report["exitCode"]), ("pass", 0))
            self.assertTrue(verifier.report_matches_schema(report, json.loads(REPORT_SCHEMA.read_text())))

            corpus = write_corpus(root / "bad", [("rights.xml", "<Rights xmlns='urn:test:roles' version='2.19'/>", {})])
            _, status = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(status, 1)

            corpus = write_corpus(root / "inconclusive", [("mxl.xml", "<document xmlns='urn:test:mxl'/>", {"newStandalone": True})])
            report, status = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(status, 3)
            self.assertEqual((report["verdict"], report["exitCode"]), ("inconclusive", 3))
            self.assertEqual(report["files"][0]["result"], "inconclusive")

            profile["runtime"]["sha256"] = "f" * 64
            with self.assertRaises(verifier.SourceError):
                verifier.verified_runtime(archive, profile)

    def test_owner_version_qname_and_corpus_tampering_are_rejected(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            runtime = verifier.verified_runtime(archive, profile)
            corpus = write_corpus(root / "owned", [
                ("owner.xml", "<MetaDataObject xmlns='urn:test:md' version='2.19'/>", {}),
                ("dcs.xml", "<DataCompositionSchema xmlns='urn:test:dcs'><name>x</name></DataCompositionSchema>", {"ownerPath": "case/owner.xml"}),
            ])
            _, status = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(status, 1)

            corpus = write_corpus(root / "wrong-root", [("wrong.xml", "<Rights xmlns='urn:test:roles' version='2.20'><setForNewObjects>true</setForNewObjects></Rights>", {"family": "dcs", "newStandalone": True})])
            report, status = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(status, 1)
            self.assertEqual(next(check for check in report["files"][0]["checks"] if check["name"] == "rootQName")["status"], "fail")

            corpus = write_corpus(root / "qname", [("owner.xml", "<MetaDataObject xmlns='urn:test:md' version='2.20' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xsi:type='missing:T'/>", {})])
            _, status = verifier.verify_corpus(corpus, profile, runtime, {"declarations": {"metadata": True}})
            self.assertEqual(status, 1)

            corpus = write_corpus(root / "text-qname", [("owner.xml", "<MetaDataObject xmlns='urn:test:md' xmlns:v8='urn:test:v8' version='2.20'><v8:Type>missing:T</v8:Type></MetaDataObject>", {})])
            _, status = verifier.verify_corpus(corpus, profile, runtime, {"declarations": {"metadata": True}})
            self.assertEqual(status, 1)

            corpus = write_corpus(root / "unlisted", [("rights.xml", "<Rights xmlns='urn:test:roles' version='2.20'><setForNewObjects>true</setForNewObjects></Rights>", {})])
            (corpus.parent / "case/extra.xml").write_text("<x/>")
            with self.assertRaisesRegex(verifier.CorpusError, "unlisted XML"):
                verifier.verify_corpus(corpus, profile, runtime, None)

    def test_report_is_deterministic(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            runtime = verifier.verified_runtime(archive, profile)
            corpus = write_corpus(root / "corpus", [("rights.xml", "<Rights xmlns='urn:test:roles' version='2.20'><setForNewObjects>true</setForNewObjects></Rights>", {})])
            first, _ = verifier.verify_corpus(corpus, profile, runtime, None)
            second, _ = verifier.verify_corpus(corpus, profile, runtime, None)
            self.assertEqual(json.dumps(first, sort_keys=True), json.dumps(second, sort_keys=True))

    def test_edt_declaration_is_line_evidence_not_xsd_coverage(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = runtime_archive(root, basic_schemas())
            profile = test_profile(archive)
            jar = edt_jar(root)
            profile["edt"] = {
                "sha256": sha(jar.read_bytes()), "symbolicName": "test", "version": "1",
                "entries": {"backend": "backend.xdto"},
                "declarations": {"metadata": {"entry": "backend", "tokens": ["urn:test:md", "MetaDataObject"]}}
            }

            def verified_runner(*_args, **_kwargs):
                return subprocess.CompletedProcess([], 0, stdout="jar verified.\n", stderr="")

            edt = verifier.verified_edt(jar, profile, runner=verified_runner)
            runtime = verifier.verified_runtime(archive, profile)
            corpus = write_corpus(root / "corpus", [("meta.xml", "<MetaDataObject xmlns='urn:test:md' version='2.20'/>", {})])
            report, status = verifier.verify_corpus(corpus, profile, runtime, edt)
            self.assertEqual(status, 3)
            self.assertEqual(report["files"][0]["coverage"], "not-covered")
            self.assertEqual(report["files"][0]["checks"][-1]["name"], "edtDeclaration")
            self.assertIn("not proof of patch build", report["sources"]["edt"]["evidenceScope"])

    def test_corpus_rejects_escape_absolute_duplicate_and_hash_mismatch(self):
        verifier = load_verifier()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            corpus = write_corpus(root, [("x.xml", "<x/>", {})])
            data = json.loads(corpus.read_text())
            for raw in ["../x.xml", "/tmp/x.xml"]:
                changed = json.loads(json.dumps(data))
                changed["cases"][0]["files"][0]["path"] = raw
                corpus.write_text(json.dumps(changed))
                with self.assertRaisesRegex(verifier.CorpusError, "unsafe corpus path"):
                    verifier._load_corpus(corpus, "test-2.20")
            changed = json.loads(json.dumps(data))
            changed["cases"][0]["files"][0]["sha256"] = "0" * 64
            corpus.write_text(json.dumps(changed))
            with self.assertRaisesRegex(verifier.CorpusError, "hash mismatch"):
                verifier._load_corpus(corpus, "test-2.20")
            changed = json.loads(json.dumps(data))
            changed["cases"][0]["files"].append(dict(changed["cases"][0]["files"][0]))
            corpus.write_text(json.dumps(changed))
            with self.assertRaisesRegex(verifier.CorpusError, "duplicate"):
                verifier._load_corpus(corpus, "test-2.20")
            (root / "alias").symlink_to(root / "case", target_is_directory=True)
            changed = json.loads(json.dumps(data))
            changed["cases"][0]["files"][0]["path"] = "alias/x.xml"
            corpus.write_text(json.dumps(changed))
            with self.assertRaisesRegex(verifier.CorpusError, "symlink"):
                verifier._load_corpus(corpus, "test-2.20")


if __name__ == "__main__":
    unittest.main()
