"""
Test cho bộ hồ sơ đấu nối (app/services/dossier_service.py).

Trọng tâm là ba thứ dễ hỏng nhất:
  1. Đánh số tên file phải khớp thứ tự các ô trên trang quản lý bán hàng.
  2. Tạo lại phải dọn sạch kết quả lần trước — giấy tờ đã xoá không được nằm lại
     rồi bị nộp nhầm.
  3. Việc dọn đó không được đụng bất cứ thứ gì ngoài thư mục do ứng dụng tạo.
"""

from __future__ import annotations

from datetime import date, timedelta

import pytest


@pytest.fixture()
def ds(db_engine, tmp_path, monkeypatch):
    """Service, với DOSSIER_DIR trỏ vào thư mục riêng của từng test.

    Cần patch tay vì hai lẽ: conftest đặt CONTRACTFORGE_DATA_ROOT ở cấp process
    mà ``_resolve_data_root()`` ưu tiên biến môi trường hơn ``cf_set_data_root``,
    nên mọi test trong cùng lần chạy dùng chung một DATA_DIR. Không cách ly thì
    thư mục hồ sơ của test này đè lên test kia và kết quả phụ thuộc thứ tự chạy.

    Phải phụ thuộc ``db_engine`` để chạy SAU nó: fixture đó gọi
    ``cf_set_data_root`` → ``_refresh_derived_paths`` và sẽ ghi đè patch này nếu
    chạy sau.
    """
    from app import paths
    from app.services import dossier_service

    monkeypatch.setattr(paths, "DOSSIER_DIR", tmp_path / "dau-noi", raising=True)
    return dossier_service


def _write(path, data: bytes = b"x"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


@pytest.fixture()
def customer_with_docs(db_session, make_customer, data_root):
    """Khách hàng có đủ CCCD hai mặt + quyết định thành lập trên đĩa."""
    from app.models.customer_document import CustomerDocument

    cust = make_customer(code="KH-010", short_name="TYT Mẫu Một",
                         legal_name="Trạm Y tế Mẫu Một")
    docs_dir = data_root / "uploads" / "customer_docs" / str(cust.id)
    specs = [
        ("cccd_front",    "truoc.jpg", "image"),
        ("cccd_back",     "sau.jpg",   "image"),
        ("establishment", "qd.pdf",    "pdf"),
    ]
    for doc_type, fname, hint in specs:
        p = _write(docs_dir / fname, b"y" * 2048)
        db_session.add(CustomerDocument(
            customer_id=cust.id, doc_type=doc_type,
            file_path=str(p), file_name=fname, mime_hint=hint))
    db_session.commit()
    return cust


# ─── Dựng kế hoạch ────────────────────────────────────────────────────────────

def test_slot_numbering_matches_portal_field_order(ds, db_session, customer_with_docs):
    """Tên file phải mang số thứ tự của ô, theo đúng thứ tự DEFAULT_SLOTS."""
    plan = ds.build_plan(db_session, customer_with_docs)

    by_key = {s.slot.key: s for s in plan.slots}
    assert by_key["cccd_front"].dest_name == "01_CCCD_mat_truoc.jpg"
    assert by_key["cccd_back"].dest_name == "02_CCCD_mat_sau.jpg"
    assert by_key["establishment"].dest_name == "03_Quyet_dinh_thanh_lap.pdf"


def test_missing_document_is_reported_not_silently_skipped(ds, db_session, make_customer):
    """Thiếu giấy tờ phải hiện ra ở kế hoạch — đó là lý do có màn hình xác nhận."""
    cust = make_customer(code="KH-999", short_name="TYT Mẫu Rỗng")
    plan = ds.build_plan(db_session, cust)

    assert not plan.is_complete
    missing_keys = {m.slot.key for m in plan.missing_required}
    assert {"cccd_front", "cccd_back", "establishment"} == missing_keys


def test_document_row_pointing_at_deleted_file_is_flagged(ds, db_session,
                                                          customer_with_docs, data_root):
    """Bản ghi còn nhưng file mất trên đĩa: phải báo, không được coi như có."""
    from app.models.customer_document import CustomerDocument

    doc = (db_session.query(CustomerDocument)
           .filter_by(customer_id=customer_with_docs.id, doc_type="cccd_front").one())
    from pathlib import Path
    Path(doc.file_path).unlink()

    plan = ds.build_plan(db_session, customer_with_docs)
    slot = next(s for s in plan.slots if s.slot.key == "cccd_front")
    assert not slot.ok
    assert "đĩa" in slot.note


def test_relative_stored_path_resolves_against_data_root(ds, tmp_path, monkeypatch):
    """Bước 5 sẽ đổi sang path tương đối — service phải hiểu sẵn cả hai kiểu."""
    from app import paths
    monkeypatch.setattr(paths, "DATA_DIR", tmp_path / "cf_data", raising=True)

    p = _write(tmp_path / "cf_data" / "uploads" / "customer_docs" / "9" / "a.pdf")
    assert ds.resolve_stored_path("uploads/customer_docs/9/a.pdf") == p
    assert ds.resolve_stored_path(str(p)) == p
    assert ds.resolve_stored_path(None) is None


def test_other_documents_land_in_a_separate_number_range(ds, db_session,
                                                         customer_with_docs, data_root):
    """Giấy tờ 'khác' không ứng với ô nào nên phải nằm ngoài dải số của các ô."""
    from app.models.customer_document import CustomerDocument

    p = _write(data_root / "uploads" / "customer_docs" /
               str(customer_with_docs.id) / "khac.pdf")
    db_session.add(CustomerDocument(
        customer_id=customer_with_docs.id, doc_type="other",
        label="Giấy uỷ quyền", file_path=str(p),
        file_name="khac.pdf", mime_hint="pdf"))
    db_session.commit()

    plan = ds.build_plan(db_session, customer_with_docs)
    extra = next(s for s in plan.slots if s.order >= ds.EXTRA_START)
    assert extra.dest_name == "90_GIAY_UY_QUYEN.pdf"
    assert not extra.slot.required


# ─── Hợp đồng: mỗi sản phẩm một ô riêng ───────────────────────────────────────

@pytest.fixture()
def two_products(make_product):
    """Hai sản phẩm — sản phẩm tạo trước chiếm dải số 04/05, sau là 06/07."""
    return make_product(code="SW-AAA", name="Sản phẩm A"),            make_product(code="SW-BBB", name="Sản phẩm B")


def _scan(data_root, name):
    return _write(data_root / "uploads" / "signed_scans" / name, b"%PDF-1.4")


def test_each_product_gets_its_own_contract_slot(ds, db_session, make_contract,
                                                 customer_with_docs, two_products,
                                                 data_root):
    """Khách hàng mua hai sản phẩm phải ra hai ô hợp đồng, không phải một."""
    from app.models.contract import ContractStatus

    a, b = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="01/A/2026", signed_pdf_path=str(_scan(data_root, "a.pdf")))
    make_contract(customer=customer_with_docs, product=b, status=ContractStatus.Signed,
                  contract_number="01/B/2026", signed_pdf_path=str(_scan(data_root, "b.pdf")))

    names = {s.dest_name for s in ds.build_plan(db_session, customer_with_docs).slots if s.ok}
    assert "04_Hop_dong_SW-AAA.pdf" in names
    assert "06_Hop_dong_SW-BBB.pdf" in names


def test_contract_without_scan_is_not_masked_by_the_other_product(
        ds, db_session, make_contract, customer_with_docs, two_products, data_root):
    """Hồi quy: HĐ sản phẩm A chưa có scan không được bị HĐ sản phẩm B che mất.

    Lỗi cũ: kế hoạch chỉ có MỘT ô hợp đồng, chọn qua dropdown, và luôn ưu tiên
    hợp đồng đã có scan. Upload scan cho hợp đồng còn lại xong, thư mục hồ sơ
    vẫn giữ nguyên hợp đồng cũ — người dùng tưởng đã nộp đủ.
    """
    from app.models.contract import ContractStatus

    a, b = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="01/A/2026", signed_pdf_path=None)
    make_contract(customer=customer_with_docs, product=b, status=ContractStatus.Signed,
                  contract_number="01/B/2026", signed_pdf_path=str(_scan(data_root, "b.pdf")))

    plan = ds.build_plan(db_session, customer_with_docs)

    missing = {m.order for m in plan.missing_required}
    assert 4 in missing, "ô hợp đồng của sản phẩm A phải báo thiếu"
    filled = {s.order: s.dest_name for s in plan.slots if s.ok}
    assert filled.get(6) == "06_Hop_dong_SW-BBB.pdf", "hợp đồng sản phẩm B phải ở ô 06"
    assert not plan.is_complete


def test_slot_number_stays_with_the_product_even_when_absent(
        ds, db_session, make_contract, customer_with_docs, two_products, data_root):
    """Khách chỉ mua sản phẩm thứ hai vẫn nhận ô 06 — số ô không dồn lên 04.

    Số ô phải ứng cố định với một ô trên trang bán hàng. Nếu dồn lên, cùng một
    số lại là loại giấy tờ khác nhau tuỳ khách hàng.
    """
    from app.models.contract import ContractStatus

    _, b = two_products
    make_contract(customer=customer_with_docs, product=b, status=ContractStatus.Signed,
                  contract_number="01/B/2026", signed_pdf_path=str(_scan(data_root, "b.pdf")))

    orders = {s.dest_name: s.order for s in ds.build_plan(db_session, customer_with_docs).slots if s.ok}
    assert orders["06_Hop_dong_SW-BBB.pdf"] == 6


def test_two_contracts_of_the_same_product_do_not_overwrite_each_other(
        ds, db_session, make_contract, customer_with_docs, two_products, data_root):
    """Ký bổ sung đợt hai cùng sản phẩm: hai file, không được trùng tên."""
    from app.models.contract import ContractStatus

    a, _ = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="01/A/2026", signed_pdf_path=str(_scan(data_root, "a1.pdf")))
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="02/A/2026", signed_pdf_path=str(_scan(data_root, "a2.pdf")))

    names = [s.dest_name for s in ds.build_plan(db_session, customer_with_docs).slots if s.ok]
    assert "04_Hop_dong_SW-AAA.pdf" in names
    assert "04_Hop_dong_SW-AAA_2.pdf" in names
    assert len(names) == len(set(names)), "tên file bị trùng"


def test_draft_contract_is_left_out(ds, db_session, make_contract, customer_with_docs,
                                    two_products):
    """Bản nháp chưa ký thì chưa có gì để nộp — không tạo ô, không báo thiếu."""
    from app.models.contract import ContractStatus

    a, _ = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Draft,
                  contract_number="01/A/2026")

    plan = ds.build_plan(db_session, customer_with_docs)
    assert plan.contracts == []
    assert all(s.order < ds.CONTRACT_BLOCK_START or s.order >= ds.EXTRA_START
               for s in plan.slots)


def test_checklist_lists_every_contract(ds, db_session, make_contract,
                                        customer_with_docs, two_products, data_root):
    """Người dùng phải thấy hồ sơ này gồm những hợp đồng nào."""
    from app.models.contract import ContractStatus

    a, b = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="01/A/2026", signed_pdf_path=str(_scan(data_root, "a.pdf")))
    make_contract(customer=customer_with_docs, product=b, status=ContractStatus.Signed,
                  contract_number="01/B/2026", signed_pdf_path=str(_scan(data_root, "b.pdf")))

    text = ds.build_checklist_text(ds.build_plan(db_session, customer_with_docs))
    assert "01/A/2026" in text and "01/B/2026" in text
    assert "SW-AAA" in text and "SW-BBB" in text


# ─── Ghi ra đĩa ───────────────────────────────────────────────────────────────

def test_materialize_copies_files_and_leaves_originals_alone(ds, db_session,
                                                             customer_with_docs):
    from app.models.customer_document import CustomerDocument

    plan = ds.build_plan(db_session, customer_with_docs)
    result = ds.materialize(plan)

    assert result.copied == 3
    dest = result.dest_dir
    assert dest.name == "KH-010_TYT_MAU_MOT"
    assert (dest / "01_CCCD_mat_truoc.jpg").exists()
    assert (dest / "_checklist.txt").exists()

    # File gốc còn nguyên — service chỉ copy.
    for doc in db_session.query(CustomerDocument).filter_by(
            customer_id=customer_with_docs.id):
        from pathlib import Path
        assert Path(doc.file_path).exists()


def test_files_are_hardlinked_so_the_dossier_costs_no_extra_disk(ds, db_session,
                                                                 customer_with_docs):
    """Thư mục hồ sơ không được nhân đôi dữ liệu: hai tên, một khối trên đĩa."""
    import os

    plan = ds.build_plan(db_session, customer_with_docs)
    result = ds.materialize(plan)

    assert result.linked == result.copied == 3
    assert result.duplicated_bytes_saved

    src = next(s for s in plan.slots if s.slot.key == "cccd_front").source_path
    dest = result.dest_dir / "01_CCCD_mat_truoc.jpg"
    assert os.stat(src).st_ino == os.stat(dest).st_ino


def test_deleting_the_dossier_never_touches_the_original(ds, db_session,
                                                         customer_with_docs):
    """Hardlink chỉ là một cái tên — xoá tên bên này, dữ liệu gốc còn nguyên."""
    import shutil

    plan = ds.build_plan(db_session, customer_with_docs)
    src = next(s for s in plan.slots if s.slot.key == "cccd_front").source_path
    original = src.read_bytes()

    result = ds.materialize(plan)
    shutil.rmtree(result.dest_dir)

    assert src.exists()
    assert src.read_bytes() == original


def test_falls_back_to_copy_when_hardlink_unsupported(ds, db_session,
                                                      customer_with_docs, monkeypatch):
    """Ổ FAT32 hoặc khác phân vùng: phải copy chứ không được hỏng."""
    import os as _os

    def _no_link(*a, **kw):
        raise OSError("hệ thống file không hỗ trợ hardlink")

    monkeypatch.setattr(_os, "link", _no_link)

    result = ds.materialize(ds.build_plan(db_session, customer_with_docs))
    assert result.copied == 3
    assert result.linked == 0
    assert not result.duplicated_bytes_saved
    assert (result.dest_dir / "01_CCCD_mat_truoc.jpg").exists()


def test_stale_dossier_is_detected_after_source_changes(ds, db_session,
                                                        customer_with_docs):
    """Upload đổi đuôi file sinh file mới — liên kết cũ giữ nội dung cũ, phải cảnh báo."""
    import os
    import time

    plan = ds.build_plan(db_session, customer_with_docs)
    ds.materialize(plan)
    assert not ds.is_stale(plan)

    # Giả lập giấy tờ được cập nhật sau khi hồ sơ đã tạo.
    src = next(s for s in plan.slots if s.slot.key == "cccd_back").source_path
    future = time.time() + 60
    os.utime(src, (future, future))

    assert ds.is_stale(ds.build_plan(db_session, customer_with_docs))


def test_rebuild_removes_stale_file_from_previous_run(ds, db_session,
                                                      customer_with_docs):
    """Giấy tờ bị xoá khỏi hệ thống không được nằm lại trong thư mục hồ sơ."""
    from app.models.customer_document import CustomerDocument

    dest = ds.materialize(ds.build_plan(db_session, customer_with_docs)).dest_dir
    assert (dest / "03_Quyet_dinh_thanh_lap.pdf").exists()

    db_session.query(CustomerDocument).filter_by(
        customer_id=customer_with_docs.id, doc_type="establishment").delete()
    db_session.commit()

    result = ds.materialize(ds.build_plan(db_session, customer_with_docs))
    assert not (dest / "03_Quyet_dinh_thanh_lap.pdf").exists()
    assert (dest / "01_CCCD_mat_truoc.jpg").exists()
    assert result.removed >= 1


def test_foreign_folder_is_never_wiped(ds, db_session, customer_with_docs):
    """Thư mục trùng tên nhưng không do ứng dụng tạo: giữ nguyên file của người dùng."""
    plan = ds.build_plan(db_session, customer_with_docs)
    plan.dest_dir.mkdir(parents=True, exist_ok=True)
    mine = plan.dest_dir / "ghi_chu_cua_toi.txt"
    mine.write_text("đừng xoá", encoding="utf-8")

    result = ds.materialize(plan)

    assert mine.exists(), "file của người dùng bị xoá mất"
    assert result.removed == 0
    assert any("không do ứng dụng tạo" in e for e in result.errors)


def test_clear_refuses_to_touch_paths_outside_dossier_dir(ds, tmp_path):
    outside = tmp_path / "khong_lien_quan"
    outside.mkdir()
    (outside / "quan_trong.txt").write_text("x", encoding="utf-8")

    with pytest.raises(ValueError):
        ds._clear_managed_files(outside)
    assert (outside / "quan_trong.txt").exists()


def test_open_refuses_paths_outside_dossier_dir(ds, tmp_path):
    with pytest.raises(ValueError):
        ds.open_in_file_manager(tmp_path)


def test_checklist_names_what_is_missing(ds, db_session, make_contract,
                                         customer_with_docs, two_products):
    """_checklist.txt là thứ người dùng đọc khi mở thư mục — phải nói rõ thiếu gì."""
    from app.models.contract import ContractStatus

    a, _ = two_products
    make_contract(customer=customer_with_docs, product=a, status=ContractStatus.Signed,
                  contract_number="01/A/2026", signed_pdf_path=None)

    text = ds.build_checklist_text(ds.build_plan(db_session, customer_with_docs))

    assert "KH-010" in text
    assert "01_CCCD_mat_truoc.jpg" in text
    assert "Hợp đồng đã ký" in text          # mục thiếu phải được gọi tên
    assert "Thiếu 1 mục bắt buộc" in text


def test_checklist_confirms_when_nothing_is_missing(ds, db_session, customer_with_docs):
    """Khách hàng đủ giấy tờ và chưa có hợp đồng nào thì không được báo thiếu."""
    text = ds.build_checklist_text(ds.build_plan(db_session, customer_with_docs))
    assert "Đủ toàn bộ mục bắt buộc" in text


def test_dossier_dir_is_readable_and_scoped(ds, db_session, make_customer):
    """Tên thư mục phải mang mã + tên KH, và luôn nằm trong DOSSIER_DIR."""
    from app import paths

    cust = make_customer(code="KH-042", short_name="TYT Mẫu Hai")
    d = ds.dossier_dir_for(cust)
    assert d.parent == paths.DOSSIER_DIR
    assert d.name == "KH-042_TYT_MAU_HAI"
