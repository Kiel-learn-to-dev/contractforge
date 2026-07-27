"""
Model: Contract — Bản ghi hợp đồng nghiệp vụ (không chỉ là file Word).

Blueprint §5.4: lưu toàn bộ dữ liệu hợp đồng, trạng thái vòng đời,
liên kết khách hàng, sản phẩm, template và file đầu ra.

Trạng thái (ContractStatus):
  Draft → Generated → Sent → Signed → Invoiced → PaidActive → Expired → Terminated

Luật chuyển trạng thái và chứng từ bắt buộc: app/services/lifecycle.py
"""

import enum
import json
from datetime import datetime
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Date, Text, Numeric, ForeignKey, Enum
from sqlalchemy.orm import relationship
from app.database import Base


class ContractStatus(str, enum.Enum):
    Draft        = "Draft"         # Đang soạn nháp
    Generated    = "Generated"     # Đã sinh file
    Sent         = "Sent"          # Đã gửi khách hàng
    Signed       = "Signed"        # Đã ký
    Active       = "Active"        # Hiệu lực (legacy — dữ liệu cũ, mới dùng PaidActive)
    Invoiced     = "Invoiced"      # Đã xuất hóa đơn
    PaidActive   = "PaidActive"    # Đã thanh toán - Đang hiệu lực
    ExpiringSoon = "ExpiringSoon"   # Sắp hết hạn (< 30 ngày)
    Expired      = "Expired"       # Đã hết hạn
    Terminated   = "Terminated"    # Đã thanh lý / kết thúc sớm


class Contract(Base):
    __tablename__ = "contracts"

    id = Column(Integer, primary_key=True, index=True)

    # --- Số hợp đồng & trạng thái ---
    contract_number = Column(String(100), unique=True, nullable=False, index=True,
                              comment="Số hợp đồng chính thức, VD: 01/ACME/2026")
    status = Column(Enum(ContractStatus), nullable=False,
                    default=ContractStatus.Draft, index=True)

    # --- Liên kết danh mục ---
    customer_id = Column(Integer, ForeignKey("customers.id"), nullable=False, index=True)
    customer = relationship("Customer", backref="contracts")

    product_id = Column(Integer, ForeignKey("products.id"), nullable=True)
    product = relationship("Product", backref="contracts")

    template_id = Column(Integer, ForeignKey("contract_templates.id"), nullable=True)
    template = relationship("ContractTemplate", backref="contracts")

    # --- Thời hạn ---
    sign_date = Column(Date, nullable=True, comment="Ngày ký hợp đồng")
    start_date = Column(Date, nullable=True, comment="Ngày hiệu lực / ngày bắt đầu dịch vụ")
    end_date = Column(Date, nullable=True, comment="Ngày hết hạn")
    month_count = Column(Integer, nullable=True, comment="Số tháng (tính từ start → end)")

    # --- Địa điểm ký ---
    sign_location = Column(String(300), nullable=True,
                           comment="Địa điểm ký — thường là tên đơn vị Bên A")

    # --- Giá trị hợp đồng ---
    unit_count = Column(Integer, nullable=True, default=1,
                        comment="Số đơn vị (gói, trạm, máy...)")
    quantity = Column(Numeric(18, 2), nullable=True,
                      comment="Số lượng = unit_count × month_count")
    unit_price_pre_vat = Column(Numeric(18, 0), nullable=True,
                                 comment="Đơn giá chưa VAT (VNĐ/đơn vị/tháng)")
    vat_rate = Column(Numeric(5, 2), nullable=True, default=0,
                      comment="% VAT, VD: 0 / 8 / 10")
    unit_price_vat = Column(Numeric(18, 0), nullable=True,
                             comment="Đơn giá đã có VAT")
    total_amount = Column(Numeric(18, 0), nullable=True,
                          comment="Tổng giá trị hợp đồng (VNĐ)")
    amount_text = Column(String(500), nullable=True,
                         comment="Số tiền bằng chữ — tự sinh từ total_amount")
    pricing_description = Column(Text, nullable=True,
                                  comment="Mô tả cách tính giá (điền vào {{PRICING_DESCRIPTION}})")

    # --- Thông tin bên B (Viettel) override nếu cần ---
    party_b_representative = Column(String(150), nullable=True,
                                     comment="Người đại diện Bên B (Viettel) cho hợp đồng này")
    party_b_authorization_number = Column(String(100), nullable=True)
    party_b_authorization_date = Column(String(50), nullable=True,
                                         comment="Lưu dạng text để linh hoạt format")

    # --- Ngày nghiệm thu / thanh lý ---
    acceptance_date = Column(Date, nullable=True, comment="Ngày ký biên bản nghiệm thu")
    liquidation_date = Column(Date, nullable=True, comment="Ngày ký biên bản thanh lý")

    # --- File đầu ra (.docx sinh tự động) ---
    output_file_path = Column(String(500), nullable=True,
                               comment="Đường dẫn file .docx đã sinh")
    output_file_name = Column(String(200), nullable=True)

    # --- File scan hợp đồng đã ký (PDF upload thủ công) ---
    signed_pdf_path = Column(String(500), nullable=True,
                              comment="Đường dẫn file PDF scan hợp đồng đã ký")
    signed_pdf_name = Column(String(200), nullable=True,
                              comment="Tên file gốc khi người dùng upload")
    signed_pdf_uploaded_at = Column(DateTime, nullable=True,
                                     comment="Thời điểm upload scan PDF")

    # --- Hóa đơn (Invoiced) ---
    invoice_date = Column(Date, nullable=True,
                          comment="Ngày xuất hóa đơn")
    invoice_pdf_path = Column(String(500), nullable=True,
                               comment="Đường dẫn file hóa đơn PDF")
    invoice_pdf_name = Column(String(200), nullable=True,
                               comment="Tên file hóa đơn gốc khi upload")
    invoice_pdf_uploaded_at = Column(DateTime, nullable=True,
                                      comment="Thời điểm upload hóa đơn")

    # --- Ủy nhiệm chi (PaidActive) ---
    payment_date = Column(Date, nullable=True,
                          comment="Ngày thanh toán theo ủy nhiệm chi")
    payment_slip_path = Column(String(500), nullable=True,
                                comment="Đường dẫn file ủy nhiệm chi (PDF hoặc ảnh)")
    payment_slip_name = Column(String(200), nullable=True,
                                comment="Tên file ủy nhiệm chi gốc khi upload")
    payment_slip_uploaded_at = Column(DateTime, nullable=True,
                                       comment="Thời điểm upload ủy nhiệm chi")

    # --- Cơ sở áp dụng (chọn khi sinh hợp đồng) ---
    # Lưu danh sách tên cơ sở (trạm chính + điểm trạm) thực sự mua trong HĐ này.
    # unit_count = số phần tử của list này.
    _selected_unit_names = Column("selected_unit_names", Text, nullable=True,
                                  comment="JSON list tên cơ sở được chọn cho hợp đồng này")

    # --- Override data (JSON) ---
    # Lưu toàn bộ dict dữ liệu đã dùng để sinh file — dễ tái tạo sau này
    _render_data = Column("render_data", Text, nullable=True,
                           comment="JSON snapshot dữ liệu khi generate")

    notes = Column(Text, nullable=True)
    is_batch = Column(Boolean, default=False,
                      comment="True nếu được tạo từ job sinh hàng loạt")
    batch_job_id = Column(String(100), nullable=True, index=True)

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    # Events
    events = relationship("ContractEvent", back_populates="contract",
                          order_by="ContractEvent.occurred_at",
                          cascade="all, delete-orphan")

    # --- Helpers ---
    @property
    def render_data(self) -> dict:
        if not self._render_data:
            return {}
        return json.loads(self._render_data)

    @render_data.setter
    def render_data(self, value: dict):
        self._render_data = json.dumps(value, ensure_ascii=False)

    @property
    def selected_unit_names(self) -> list:
        if not self._selected_unit_names:
            return []
        try:
            return json.loads(self._selected_unit_names)
        except (ValueError, TypeError):
            return []

    @selected_unit_names.setter
    def selected_unit_names(self, value):
        self._selected_unit_names = json.dumps(list(value or []), ensure_ascii=False)

    # ── Hết hạn: dẫn xuất từ end_date, không phải trạng thái lưu trong DB ────
    # Import cục bộ: lifecycle import ContractStatus từ chính module này, nên
    # import ở đầu file sẽ thành vòng tròn.

    @property
    def days_to_expiry(self):
        """Số ngày còn lại tới hạn. Âm nếu đã quá hạn, None nếu không có hạn."""
        from app.services.lifecycle import days_to_expiry
        return days_to_expiry(self.end_date)

    @property
    def expiry_bucket(self):
        """'expired' | 'critical' | 'warning' | 'info' | None — mức khẩn của hạn."""
        from app.services.lifecycle import expiry_bucket
        return expiry_bucket(self.end_date)

    @property
    def is_expiring_soon(self) -> bool:
        """Còn hiệu lực nhưng sẽ hết hạn trong ≤30 ngày."""
        from app.services.lifecycle import is_expiring_soon
        return is_expiring_soon(self.end_date)

    def __repr__(self):
        return f"<Contract {self.contract_number} [{self.status}]>"
