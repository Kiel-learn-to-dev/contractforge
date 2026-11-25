import json
import time

from fastapi import FastAPI
from fastapi.testclient import TestClient

from app.live_app import LiveDevelopmentMiddleware
from run_live import _reloadable_file


def test_live_revision_endpoint_is_no_store_and_has_worker_revision():
    app = FastAPI()
    wrapped = LiveDevelopmentMiddleware(app, revision="build-123")

    response = TestClient(wrapped).get("/__live/revision")

    assert response.status_code == 200
    assert response.json() == {
        "revision": "build-123",
        "worker_revision": "build-123",
        "data_revision": "",
    }
    assert response.headers["cache-control"].startswith("no-store")
    assert TestClient(wrapped).head("/__live/revision").status_code == 404


def test_live_revision_changes_when_database_or_wal_metadata_changes(tmp_path):
    app = FastAPI()
    db_path = tmp_path / "contracts.db"
    db_path.write_bytes(b"database")
    wrapped = LiveDevelopmentMiddleware(app, revision="build-123", db_path=db_path)
    client = TestClient(wrapped)

    first = client.get("/__live/revision").json()
    assert first["revision"] == f"build-123:{first['data_revision']}"
    assert str(tmp_path) not in json.dumps(first)

    time.sleep(0.002)
    db_path.write_bytes(b"database updated")
    second = client.get("/__live/revision").json()
    assert second["data_revision"] != first["data_revision"]
    assert second["revision"] != first["revision"]

    wal_path = tmp_path / "contracts.db-wal"
    wal_path.write_bytes(b"wal changes")
    third = client.get("/__live/revision").json()
    assert third["data_revision"] != second["data_revision"]


def test_html_initial_revision_matches_polling_revision_without_path_disclosure(tmp_path):
    app = FastAPI()

    @app.get("/page")
    def page():
        from fastapi.responses import HTMLResponse
        return HTMLResponse("<html><body>hello</body></html>")

    db_path = tmp_path / "private" / "contracts.db"
    db_path.parent.mkdir()
    db_path.write_bytes(b"database")
    client = TestClient(LiveDevelopmentMiddleware(app, revision="worker", db_path=db_path))

    html = client.get("/page").text
    poll = client.get("/__live/revision").json()

    assert f'data-revision="{poll["revision"]}"' in html
    assert str(tmp_path) not in html
    assert str(tmp_path) not in json.dumps(poll)


def test_live_middleware_injects_dev_badge_and_script_only_into_html():
    app = FastAPI()

    @app.get("/page")
    def page():
        from fastapi.responses import HTMLResponse
        return HTMLResponse("<html><body><h1>hello</h1></body></html>")

    @app.get("/file")
    def file():
        from fastapi.responses import Response
        return Response(
            b"<html>document bytes</html>",
            media_type="text/html",
            headers={"Content-Disposition": "attachment; filename=document.html"},
        )

    @app.get("/stream")
    def stream():
        from fastapi.responses import StreamingResponse
        return StreamingResponse(
            iter([b"<html>", b"streamed document</html>"]),
            media_type="text/html",
        )

    client = TestClient(LiveDevelopmentMiddleware(app, revision="r1"))
    page_response = client.get("/page")
    head_response = client.head("/page")
    file_response = client.get("/file")
    stream_response = client.get("/stream")

    assert 'src="/__live/script.js"' in page_response.text
    assert 'src="/__live/script.js"' not in head_response.text
    assert "<html>document bytes</html>" == file_response.text
    assert "<html>streamed document</html>" == stream_response.text
    assert 'src="/__live/script.js"' not in file_response.text
    assert 'src="/__live/script.js"' not in stream_response.text


def test_dev_script_defers_reload_when_form_is_dirty():
    app = FastAPI()
    client = TestClient(LiveDevelopmentMiddleware(app, revision="r1"))
    response = client.get("/__live/script.js")

    assert response.status_code == 200
    assert 'target.form' in response.text
    assert "window.confirm" in response.text
    assert "window.location.reload()" in response.text
    assert "script.dataset.revision" in response.text


def test_live_watcher_tracks_source_and_web_assets_only():
    assert _reloadable_file(None, "ContractForge/app/service.py")
    assert _reloadable_file(None, "ContractForge/templates/dashboard/index.html")
    assert _reloadable_file(None, "ContractForge/static/app.css")
    assert _reloadable_file(None, "ContractForge/static/app.js")
    assert not _reloadable_file(None, "ContractForge/assets/template.docx")
    assert not _reloadable_file(None, "ContractForge/app/__pycache__/service.pyc")
