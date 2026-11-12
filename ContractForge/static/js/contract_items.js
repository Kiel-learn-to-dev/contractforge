// static/js/contract_items.js — bảng dòng giá trị dùng chung cho biểu mẫu hợp đồng
// và báo giá. Chỉ gom dữ liệu, gửi lên máy chủ và hiển thị kết quả; không tính tiền.
// Biểu mẫu cần có các phần tử: sel-product, sel-customer, inp-units, inp-months-time,
// tbl-items (tbody), addon-picker, price-error, sum-pre, sum-vat, sum-total, btn-save,
// inp-items-json, và form có thuộc tính data-items-form.

function gid(id) { return document.getElementById(id); }

// ─── TIỀN: JS chỉ gom dữ liệu, gửi đi và hiển thị kết quả ─────────────────────
// Không có phép nhân, chia hay làm tròn tiền trong phần này. Mọi số tiền (thành
// tiền, thuế, tổng, số bằng chữ, mô tả cách tính) do POST /contracts/price-preview
// trả về, đi cùng đường tính với lúc lưu hợp đồng.
function fmtN(n) { return Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g,'.'); }
function parseN(s) { return parseFloat(String(s).replace(/\./g,'').replace(',','.'))||0; }

// Chuỗi số máy → hiển thị để người dùng đọc và sửa. Số nguyên có dấu chấm nghìn;
// số thập phân dùng dấu phẩy ("82,5") để parseN đọc lại đúng khi gửi đi.
function showNum(v) {
  if (v === null || v === undefined || v === '') return '';
  var s = String(v);
  if (/^-?\d+$/.test(s)) return fmtN(Number(s));
  return /^-?\d+\.\d+$/.test(s) ? s.replace('.', ',') : s;
}
function setText(id, text) { var el = gid(id); if (el) el.textContent = text; }
function setPriceError(msg) { setText('price-error', msg || ''); }
function setSave(on) { var b = gid('btn-save'); if (b) b.disabled = !on; }

var _lastOptions = null;   // kết quả gần nhất của /contracts/product-options
var _optSeq = 0;           // chỉ áp dụng kết quả của yêu cầu mới nhất
var _previewTimer = null;
// Dòng đã nhập trước đó (form bị trả lại do lỗi): dựng lại bảng từ đây.
var _prefillItems = (function() {
  var el = gid('inp-items-json');
  if (!el || !el.value) return null;
  try { var a = JSON.parse(el.value); return Array.isArray(a) && a.length ? a : null; }
  catch (e) { return null; }
})();

function itemsBody() { var t = gid('tbl-items'); return t ? t.querySelector('tbody') : null; }

// Trạng thái các dòng đang có trên màn hình, kể cả ô người dùng đã sửa tay.
function readRowState() {
  var state = {};
  var body = itemsBody(); if (!body) return state;
  body.querySelectorAll('tr[data-product-id]').forEach(function(tr) {
    var q = tr.querySelector('.inp-qty'), p = tr.querySelector('.inp-price'), v = tr.querySelector('.inp-vat');
    state[tr.dataset.productId] = {
      qty: q.value, qtyManual: !!q.dataset.manual,
      price: p.value, priceManual: !!p.dataset.manual,
      vat: v.value, vatManual: !!v.dataset.manual
    };
  });
  return state;
}

function checkedAddonIds() {
  var ids = {};
  document.querySelectorAll('#addon-picker input[type=checkbox][data-pid]').forEach(function(cb) {
    if (cb.checked) ids[cb.dataset.pid] = true;
  });
  return ids;
}

// Ô đã sửa tay thì giữ giá trị người dùng; còn lại lấy mặc định mới từ máy chủ.
function keepOrDefault(saved, key, fallback) {
  return (saved && saved[key + 'Manual']) ? saved[key] : fallback;
}

// raw=true: giữ nguyên chuỗi (thuế suất được đọc bằng parseFloat, cần dấu chấm).
function makeInput(cls, value, manual, raw) {
  var input = document.createElement('input');
  input.type = 'text';
  input.className = 'fc fc-sm ' + cls;
  input.value = raw ? String(value === null || value === undefined ? '' : value) : showNum(value);
  if (manual) input.dataset.manual = '1';
  return input;
}

function makeCell(cls, text) {
  var td = document.createElement('td');
  if (cls) td.className = cls;
  td.textContent = text || '';
  return td;
}

function makeRow(p, kind, saved) {
  var tr = document.createElement('tr');
  tr.dataset.productId = p.id;
  tr.dataset.kind = kind;
  tr.dataset.name = p.name;
  tr.appendChild(makeCell('fw-semibold', p.name));
  tr.appendChild(makeCell('', p.unit_label));
  var qtyCell = document.createElement('td');
  qtyCell.appendChild(makeInput('inp-qty text-end',
    keepOrDefault(saved, 'qty', p.default_quantity), !!(saved && saved.qtyManual)));
  tr.appendChild(qtyCell);
  var priceCell = document.createElement('td');
  priceCell.appendChild(makeInput('inp-price text-end',
    keepOrDefault(saved, 'price', p.price), !!(saved && saved.priceManual)));
  tr.appendChild(priceCell);
  var vatCell = document.createElement('td');
  vatCell.appendChild(makeInput('inp-vat text-end',
    keepOrDefault(saved, 'vat', p.vat), !!(saved && saved.vatManual), true));
  tr.appendChild(vatCell);
  tr.appendChild(makeCell('text-end cell-pre', ''));
  tr.appendChild(makeCell('text-end cell-vat', ''));
  tr.appendChild(makeCell('text-end cell-total', ''));
  return tr;
}

// Dựng lại bảng dòng và danh sách gói bổ sung từ kết quả của máy chủ.
function buildTable(d, fresh) {
  var body = itemsBody(); if (!body || !d || !d.main) return;
  var prev = fresh ? {} : readRowState();
  var checked = fresh ? {} : checkedAddonIds();
  if (_prefillItems) {
    _prefillItems.forEach(function(it) {
      var key = String(it.product_id);
      prev[key] = {
        qty: showNum(it.quantity), qtyManual: true,
        price: showNum(it.unit_price_pre_vat), priceManual: true,
        vat: String(it.vat_rate), vatManual: true
      };
      checked[key] = true;
    });
    _prefillItems = null;
  }

  body.innerHTML = '';
  body.appendChild(makeRow(d.main, 'main', prev[String(d.main.id)]));

  var picker = gid('addon-picker');
  if (picker) picker.innerHTML = '';
  if (picker && !d.addons.length) {
    var none = document.createElement('span');
    none.className = 'text-muted small';
    none.textContent = 'Không có gói bổ sung nào cùng nhóm.';
    picker.appendChild(none);
  }
  d.addons.forEach(function(a) {
    var key = String(a.id);
    var on = !!checked[key];
    if (picker) {
      var label = document.createElement('label');
      label.className = 'd-flex align-items-center gap-2 small';
      label.style.cursor = 'pointer';
      var cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = on;
      cb.dataset.pid = key;
      cb.addEventListener('change', function() {
        buildTable(_lastOptions, false);
        refreshPreview();
      });
      var span = document.createElement('span');
      span.textContent = a.name + (a.unit_label ? ' (' + a.unit_label + ')' : '');
      label.appendChild(cb);
      label.appendChild(span);
      picker.appendChild(label);
    }
    if (on) body.appendChild(makeRow(a, 'addon', prev[key]));
  });
}

function clearSums() {
  document.querySelectorAll('#tbl-items tbody .cell-pre, #tbl-items tbody .cell-vat, #tbl-items tbody .cell-total')
    .forEach(function(td) { td.textContent = ''; });
  ['sum-pre', 'sum-vat', 'sum-total'].forEach(function(id) { setText(id, ''); });
}

function clearPriceTable() {
  var body = itemsBody(); if (body) body.innerHTML = '';
  var picker = gid('addon-picker'); if (picker) picker.innerHTML = '';
  _lastOptions = null;
  clearSums();
  setPriceError('');
  setSave(false);
}

// Lấy gói chính + gói bổ sung từ máy chủ. fresh=true: bỏ mọi ô đã sửa tay.
function loadProductOptions(fresh) {
  var sel = gid('sel-product');
  var pid = sel ? sel.value : '';
  if (!pid) { clearPriceTable(); return; }
  var custEl = gid('sel-customer'), unitsEl = gid('inp-units');
  var url = '/contracts/product-options?product_id=' + encodeURIComponent(pid)
          + '&customer_id=' + encodeURIComponent((custEl && custEl.value) || '0')
          + '&unit_count=' + encodeURIComponent((unitsEl && unitsEl.value) || '1');
  var seq = ++_optSeq;
  fetch(url).then(function(r) { return r.json(); }).then(function(d) {
    if (seq !== _optSeq) return;
    // Mẫu hợp đồng và văn bản kèm theo: chỉ thay khi đổi gói (fresh) hoặc ô mẫu còn trống,
    // để đổi khách hàng không làm mất lựa chọn đang có. Trang báo giá không có hàm này.
    if (d && d.templates && typeof applyTemplateOptions === 'function') {
      var tplEl = gid('sel-template');
      if (fresh === true || !(tplEl && tplEl.value)) applyTemplateOptions(d.templates, fresh === true);
    }
    if (!d || !d.main) { clearPriceTable(); return; }
    _lastOptions = d;
    buildTable(d, !!fresh);
    refreshPreview();
  }).catch(function() {
    if (seq === _optSeq) { setPriceError('Không tải được danh mục gói.'); setSave(false); }
  });
}

// Đọc các dòng trên màn hình thành dữ liệu gửi máy chủ. Ô tiền người dùng gõ
// (kiểu "500.000") đổi về số máy bằng parseN; ô thuế dùng parseFloat.
function collectItems() {
  var out = [];
  document.querySelectorAll('#tbl-items tbody tr[data-product-id]').forEach(function(tr) {
    out.push({
      product_id: Number(tr.dataset.productId),
      description: tr.dataset.name || '',
      quantity: String(parseN(tr.querySelector('.inp-qty').value)),
      unit_price_pre_vat: String(parseN(tr.querySelector('.inp-price').value)),
      vat_rate: String(parseFloat(tr.querySelector('.inp-vat').value) || 0)
    });
  });
  return out;
}

function applyPreview(d) {
  document.querySelectorAll('#tbl-items tbody tr[data-product-id]').forEach(function(tr, i) {
    var line = d.lines[i];
    if (!line) return;
    tr.querySelector('.cell-pre').textContent = showNum(line.amount_pre_vat);
    tr.querySelector('.cell-vat').textContent = showNum(line.vat_amount);
    tr.querySelector('.cell-total').textContent = showNum(line.amount_total);
  });
  setText('sum-pre', showNum(d.totals.pre_vat));
  setText('sum-vat', showNum(d.totals.vat));
  setText('sum-total', showNum(d.totals.total));
  var wEl = gid('inp-amttext');
  if (wEl && !wEl.dataset.manual) wEl.value = d.totals.amount_text || '';
  var dEl = gid('inp-pricing-desc');
  if (dEl && !dEl.dataset.manual) dEl.value = d.description || '';
}

function doPreview() {
  var items = collectItems();
  if (!items.length) { clearSums(); setPriceError(''); setSave(false); return; }
  var mEl = gid('inp-months-time');
  var months = parseN(mEl ? mEl.value : '');
  fetch('/contracts/price-preview', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({month_count: months, items: items})
  }).then(function(r) {
    return r.json().then(function(d) { return {ok: r.ok, d: d}; });
  }).then(function(res) {
    if (!res.ok) {
      clearSums();
      setPriceError((res.d && res.d.error) || 'Không tính được tiền.');
      setSave(false);
      return;
    }
    setPriceError('');
    applyPreview(res.d);
    setSave(true);
  }).catch(function() {
    setPriceError('Không kết nối được tới máy chủ.');
    setSave(false);
  });
}

// Gom các lần gọi liên tiếp (gõ số) thành một yêu cầu tới máy chủ.
function refreshPreview() {
  clearTimeout(_previewTimer);
  _previewTimer = setTimeout(doPreview, 250);
}

// Ô số trong bảng: người dùng gõ là đánh dấu "sửa tay", để lần tải sau không ghi đè.
var _tblItems = gid('tbl-items');
if (_tblItems) _tblItems.addEventListener('input', function(e) {
  var t = e.target;
  if (!t || !t.classList) return;
  if (t.classList.contains('inp-qty') || t.classList.contains('inp-price') || t.classList.contains('inp-vat')) {
    t.dataset.manual = '1';
    refreshPreview();
  }
});

// Số cơ sở (chỉ sửa được khi không có danh sách cơ sở): tải lại gói và số lượng mặc định.
var _unitsInput = gid('inp-units');
if (_unitsInput) _unitsInput.addEventListener('input', function() {
  this.dataset.manualUnits = '1';
  loadProductOptions(false);
});

var _wEl = gid('inp-amttext');    if (_wEl) _wEl.addEventListener('input', function() { this.dataset.manual = this.value ? '1' : ''; });
var _dEl = gid('inp-pricing-desc'); if (_dEl) _dEl.addEventListener('input', function() { this.dataset.manual = this.value ? '1' : ''; });

// Trước khi gửi: ghi dòng vào trường ẩn items_json. Không có dòng nào thì không gửi.
var _form = document.querySelector('form[data-items-form]');
if (_form) _form.addEventListener('submit', function(e) {
  var items = collectItems();
  gid('inp-items-json').value = JSON.stringify(items);
  if (!items.length) {
    e.preventDefault();
    setPriceError('Chọn sản phẩm trước khi tạo hợp đồng.');
  }
});

