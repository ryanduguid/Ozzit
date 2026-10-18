// Authoring-only builder. Requires the bundled @oai/artifact-tool runtime.
// Run from the repository root; the exported workbook has no runtime dependency.
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { SpreadsheetFile, Workbook } from '@oai/artifact-tool';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.join(root, 'templates', 'practice-calculators.xlsx');
const previewDir = process.env.OZZIT_PREVIEW_DIR;
const book = Workbook.create();
const money = '#,##0.00;(#,##0.00);"-"';
const pct = '0.0%;(0.0%);"-"';
const blue = '#2455A4';
const purple = '#66023C';
book.colorScheme = {name:'Ozzit',themeColors:{dk1:'#000000',lt1:'#FFFFFF',dk2:'#000000',lt2:'#FFFFF0',
  accent1:purple,accent2:'#990024',accent3:'#E5E4E2',accent4:'#6E6862',accent5:'#B1AFAD',accent6:'#DCDAD7',
  hlink:purple,folHlink:'#990024'}};
const sheets = new Map();
const cell = (s, a, v) => { s.getRange(a).values = [[v === '' ? null : v]]; };
const formula = (s, a, v) => { s.getRange(a).formulas = [[v]]; };
const col = n => { let t = ''; for (; n; n = Math.floor((n - 1) / 26)) t = String.fromCharCode(65 + (n - 1) % 26) + t; return t; };
function input(s, a, v, format = money) {
  cell(s, a, v);
  s.getRange(a).format.font.color = blue;
  s.getRange(a).format.fill = '#E5E4E2';
  s.getRange(a).setNumberFormat(format);
}
function sheet(name, title, last = 'N', rows = 60) {
  const s = book.worksheets.add(name);
  sheets.set(name, s);
  s.showGridLines = false;
  const r = s.getRange(`B2:${last}${rows}`);
  r.format.font = { name: 'Arial', size: 10, color: '#000000' };
  r.format.rowHeight = 21;
  r.format.columnWidth = 15;
  r.format.verticalAlignment = 'center';
  s.getRange(`B2:B${rows}`).format.columnWidth = 30;
  cell(s, 'B2', title);
  s.getRange('B2').format.font = { name: 'Arial', size: 15, bold: true, color: purple };
  cell(s, 'B3', 'Illustrative AUD inputs. Blue cells are editable. Replace examples before use.');
  s.getRange('B3').format.font.italic = true;
  s.getRange(`B2:${last}2`).format.borders = { bottom: { style: 'thin', color: purple } };
  return s;
}
function header(s, row, labels, start = 2) {
  const r = s.getRange(`${col(start)}${row}:${col(start + labels.length - 1)}${row}`);
  r.values = [labels];
  r.format = { fill: purple, font: { name: 'Arial', size: 10, bold: true, color: '#FFFFF0' },
    horizontalAlignment: 'center', verticalAlignment: 'center', wrapText: true, rowHeight: 42 };
}
function pair(s, row, label, value, format = money) {
  cell(s, `B${row}`, label); input(s, `D${row}`, value, format);
}
function result(s, row, label, f, format = money) {
  cell(s, `B${row}`, label); formula(s, `D${row}`, f);
  s.getRange(`D${row}`).setNumberFormat(format);
  s.getRange(`B${row}:D${row}`).format.font.bold = true;
}
function flag(s, address, good = 'Ready') {
  s.getRange(address).conditionalFormats.add('expression', {
    formula: `AND(${address.split(':')[0]}<>"",${address.split(':')[0]}<>"${good}")`,
    format: { fill: '#F0D9DE', font: { color: '#990024', bold: true } },
  });
  s.getRange(address).format.horizontalAlignment = 'center';
}
function note(s, address, text) { cell(s, address, text); s.getRange(address).format.font.italic = true; }
const names = ['Read me', 'Allocation', 'Contractor', 'Vendor finance', 'Mixed loan', 'Rental',
  'Packaging', 'Amortisation', 'Timeline', 'Deed register', 'Carry forward',
  'Loan offers', 'Break even', 'Working capital'];
for (const name of names) sheet(name, name === 'Read me' ? 'Ozzit practice calculators' : name,
  name === 'Mixed loan' ? 'AQ' : name === 'Timeline' ? 'AF' : name === 'Amortisation' ? 'U' : name === 'Allocation' ? 'T' : 'N',
  name === 'Mixed loan' ? 54 : 48);

{
  const s = sheets.get('Read me');
  cell(s, 'B5', 'Thirteen standalone modelling workflows using ordinary Excel formulas.');
  cell(s, 'B7', 'Open a tab and replace its fabricated inputs. No Ozzit LAMBDA import is needed.');
  cell(s, 'B9', 'Tax treatment, rates, eligibility and legal deadlines must be supplied by the reviewer.');
  cell(s, 'B10', 'This workbook does not reproduce the purchased BTS workbooks or their hidden rules.');
  header(s, 13, ['Calculator', 'What it calculates'], 2);
  s.getRange('C13:C30').format.columnWidth = 100;
  const rows = [
    ['Allocation', 'Allocate one budget across 20 people or jobs equally, by salary, or by eight weighted factors.'],
    ['Contractor', 'Compare employer cost with contractor revenue per billable week and hour.'],
    ['Vendor finance', 'Roll annual vendor debt forward using dividends, a cash reserve and own funds.'],
    ['Mixed loan', 'Allocate interest, fees and repayments across six uses and 36 dated events.'],
    ['Rental', 'Reconcile rent, operating costs, capital spending, three loans and up to five owners.'],
    ['Packaging', 'Compare employee cash pay and employer cost for up to five benefits, using supplied tax amounts.'],
    ['Amortisation', 'Allocate 11 cost cohorts across 11 annual periods with a supplied early write-off year.'],
    ['Timeline', 'Show 12 dated intervals across 24 months and measure overlap with a reporting window.'],
    ['Deed register', 'Record deed evidence for 20 trusts and flag vesting dates within five calendar years.'],
    ['Carry forward', 'Consume supplied annual capacity and five expiring buckets, oldest first.'],
    ['Loan offers', 'Compare eight fixed-rate offers including balloons, fees and a supplied rate shock.'],
    ['Break even', 'Calculate contribution, break-even units, target profit and capacity constraints.'],
    ['Working capital', 'Estimate cash released by changes in receivable, inventory and payable days.'],
  ];
  s.getRange('B14:C26').values = rows;
  note(s, 'B29', 'Sources and coverage: docs/between-spreadsheets.md and docs/practice-improvements.md.');
  note(s, 'B31', 'Sample rates and capacities are modelling assumptions, not statutory rates or limits.');
  note(s, 'B33', 'Fixed ranges: extend formulas and native checks if you add rows. Inputs remain unrounded.');
  s.tabColor = '#E5E4E2';
}
{
  const s = sheets.get('Allocation');
  pair(s, 5, 'Total employer budget', 12000);
  pair(s, 6, 'Super cost / cash bonus', 0.12, pct);
  pair(s, 7, 'Payroll cost / bonus + super', 0.05, pct);
  pair(s, 8, 'Method: 1 equal, 2 salary, 3 score', 3, '0');
  s.getRange('D8').dataValidation = { rule: { type: 'list', values: ['1', '2', '3'] } };
  header(s, 5, Array.from({ length: 8 }, (_, i) => `Factor ${i + 1}`), 10);
  for (let c = 10; c <= 17; c++) input(s, `${col(c)}6`, 1, '0.00');
  note(s, 'J8', 'Scores use a common scale. Factor weights are relative, nonnegative inputs.');
  result(s, 9, 'Input status', '=IF(AND(COUNT(D5:D8)=4,MIN(D5:D7)>=0,D8=INT(D8),D8>=1,D8<=3,OR(D8<>3,AND(COUNT(J6:Q6)=8,MIN(J6:Q6)>=0)),COUNTA(B14:B33)>0,COUNTIF(T14:T33,"Ready")=COUNTA(B14:B33)),IF(SUM(M14:M33)>0,"Ready","Zero allocation weight"),"Check inputs")', '@');
  result(s, 10, 'Cash bonus pool', '=IF(D9="Ready",D5/((1+D6)*(1+D7)),NA())');
  header(s, 13, ['Person / job', 'Salary / base', ...Array.from({length:8},(_,i)=>`Score ${i+1}`),
    'Weighted score', 'Allocation weight', 'Cash bonus', 'Super cost', 'Payroll cost', 'Total cost', 'Withholding', 'Net bonus', 'Input status']);
  for (let r = 14; r <= 33; r++) {
    input(s, `B${r}`, r < 17 ? `Example ${r - 13}` : '', '@');
    input(s, `C${r}`, r < 17 ? (r - 13) * 50000 : 0);
    for (let c = 4; c <= 11; c++) input(s, `${col(c)}${r}`, r < 17 ? r - 13 : 0, '0.00');
    input(s, `R${r}`, 0);
    formula(s, `T${r}`, `=IF(B${r}="","",IF(AND(OR($D$8=1,AND($D$8=2,COUNT(C${r})=1,C${r}>=0),AND($D$8=3,COUNT(D${r}:K${r})=8,MIN(D${r}:K${r})>=0)),COUNT(R${r})=1,R${r}>=0,SUMPRODUCT(--($B$14:$B$33=B${r}))=1),"Ready","Check inputs"))`);
    formula(s, `L${r}`, `=IF(B${r}="","",IF($D$8=3,IF(T${r}="Ready",SUMPRODUCT(D${r}:K${r},$J$6:$Q$6),NA()),""))`);
    formula(s, `M${r}`, `=IF(B${r}="","",IF(AND(T${r}="Ready",$D$8>=1,$D$8<=3,$D$8=INT($D$8)),CHOOSE($D$8,1,C${r},L${r}),NA()))`);
    formula(s, `N${r}`, `=IF(B${r}="","",IF($D$9="Ready",$D$10*M${r}/SUM($M$14:$M$33),NA()))`);
    formula(s, `O${r}`, `=IF(B${r}="","",N${r}*$D$6)`);
    formula(s, `P${r}`, `=IF(B${r}="","",(N${r}+O${r})*$D$7)`);
    formula(s, `Q${r}`, `=IF(B${r}="","",SUM(N${r}:P${r}))`);
    formula(s, `S${r}`, `=IF(B${r}="","",IF(R${r}<=N${r},N${r}-R${r},NA()))`);
  }
  for (const c of ['N','O','P','Q','R','S']) s.getRange(`${c}14:${c}35`).setNumberFormat(money);
  cell(s, 'B35', 'Total');
  for (const c of ['N','O','P','Q','R','S']) formula(s, `${c}35`, `=SUM(${c}14:${c}33)`);
  flag(s, 'D9'); flag(s, 'T14:T33');
  s.getRange('T14:T33').format.horizontalAlignment = 'center';
  note(s, 'B38', 'Set both on-cost rates to zero for ordinary overhead allocations. Withholding is supplied, not calculated.');
  s.freezePanes.freezeRows(13); s.freezePanes.freezeColumns(2);
}
{
  const s = sheets.get('Contractor');
  const rows = [['Employee cash salary',100000],['Employee employer super',12000],['Employee other on-costs',8000],
    ['Contractor retirement saving',12000],['Contractor annual overheads',10000],['Contractor insurance',2000],
    ['Billable weeks per year',42],['Billable hours per week',35]];
  rows.forEach(([label,value],i)=>pair(s,i+5,label,value,i>=6?'0.00':money));
  result(s,14,'Input status','=IF(AND(COUNT(D5:D12)=8,MIN(D5:D10)>=0,D11>0,D12>0),"Ready","Check inputs")','@');
  result(s,16,'Annual employer cost','=IF(D14="Ready",SUM(D5:D7),NA())');
  result(s,17,'Contractor annual revenue','=IF(D14="Ready",D5+SUM(D8:D10),NA())');
  result(s,18,'Revenue per billable week','=D17/D11');
  result(s,19,'Revenue per billable hour','=D18/D12');
  note(s,'F5','Amounts exclude GST and personal income tax.');
  note(s,'F7','Retirement saving is entered separately for each arrangement.');
  note(s,'F9','Billable weeks exclude leave and time that cannot be invoiced.');
  note(s,'F11','This comparison does not determine employment status.');
  flag(s,'D14');
}
{
  const s=sheets.get('Vendor finance');
  pair(s,5,'Initial vendor debt',100000);
  note(s,'F5','Annual interest is charged on opening debt. Cash is paid at year end.');
  note(s,'F7','The reserve is a supplied cash amount, not an income tax calculation.');
  header(s,10,['Year','Opening debt','Annual rate','Interest','Dividends','Cash reserve','Own funds','Cash available','Debt payment','Closing debt','Excess cash','Input status']);
  for(let r=11;r<=20;r++) {
    cell(s,`B${r}`,r-10); input(s,`D${r}`,0.06,pct); input(s,`F${r}`,14000); input(s,`G${r}`,3500); input(s,`H${r}`,2000);
    formula(s,`C${r}`,r===11?'=$D$5':`=K${r-1}`);
    formula(s,`M${r}`,`=IF(AND(COUNT(D${r},F${r}:H${r},$D$5)=5,MIN(D${r},F${r}:H${r},$D$5)>=0,G${r}<=F${r}${r>11?`,M${r-1}="Ready"`:''}),"Ready","Check inputs")`);
    formula(s,`E${r}`,`=IF(M${r}="Ready",C${r}*D${r},NA())`);
    formula(s,`I${r}`,`=IF(M${r}="Ready",F${r}-G${r}+H${r},NA())`);
    formula(s,`J${r}`,`=MIN(C${r}+E${r},I${r})`);
    formula(s,`K${r}`,`=C${r}+E${r}-J${r}`); formula(s,`L${r}`,`=I${r}-J${r}`);
  }
  s.getRange('C11:L20').setNumberFormat(money); s.getRange('D11:D20').setNumberFormat(pct);
  flag(s,'M11:M20'); result(s,23,'Debt after ten years','=K20');
}
{
  const s=sheets.get('Mixed loan');
  pair(s,5,'Opening date',new Date(Date.UTC(2026,6,1)),'d-mmm-yyyy');
  pair(s,6,'Annual simple interest rate',0.06,pct); pair(s,7,'Days per interest year',365,'0');
  header(s,5,Array.from({length:6},(_,i)=>`Use ${i+1}`),6);
  for(let c=6;c<=11;c++) input(s,`${col(c)}6`,c===6?60000:c===7?40000:0);
  note(s,'F8','Six opening balances above. Uses are labels, not tax classifications.');
  note(s,'B10','Interest accrues before each event. Draws occur before proportional repayments and fee allocation.');
  note(s,'B11','Interest and fees are reported separately and are not capitalised. Same-day events accrue zero days.');
  note(s,'B14','Interest stops at the final event date. Use a zero-cash event at the required cut-off.');
  result(s,12,'Final principal','=SUM(X51:AC51)');
  result(s,13,'Total allocated interest','=SUM(R16:W51)');
  cell(s,'F12','Input status'); formula(s,'H12','=IF(COUNTIF(AQ16:AQ51,"Ready")=36,"Ready","Check events")'); flag(s,'H12');
  cell(s,'F13','Unapplied repayments'); formula(s,'H13','=SUM(AP16:AP51)'); s.getRange('H13').setNumberFormat(money);
  header(s,15,['Event date','Repayment',...Array.from({length:6},(_,i)=>`Draw ${i+1}`),'Fee','Days',
    ...['Opening','Interest','Closing','Fee share','Repayment share'].flatMap(x=>Array.from({length:6},(_,i)=>`${x} ${i+1}`)),
    'Unapplied cash','Input status']);
  for(let r=16;r<=51;r++) {
    const i=r-16;
    input(s,`B${r}`,new Date(Date.UTC(2026,6+Math.floor(i/3),1+(i%3)*10)),'d-mmm-yyyy');
    input(s,`C${r}`,i===1?10000:0);
    for(let c=4;c<=9;c++) input(s,`${col(c)}${r}`,i===2&&c===5?10000:0);
    input(s,`J${r}`,i===1?100:0);
    const prior=r===16?'$D$5':`B${r-1}`;
    formula(s,`AQ${r}`,`=IF(AND(COUNT(B${r}:J${r})=9,COUNT($D$5:$D$7)=3,COUNT($F$6:$K$6)=6,MIN(C${r}:J${r},$D$6,$F$6:$K$6)>=0,$D$7>0,B${r}>=${prior},B${r}=INT(B${r}),OR(SUM(L${r}:Q${r},D${r}:I${r})>0,J${r}=0),${r>16?`AQ${r-1}="Ready"`:'$D$5=INT($D$5)'}),"Ready","Check inputs")`);
    formula(s,`K${r}`,`=IF(AQ${r}="Ready",B${r}-${prior},NA())`);
    for(let u=0;u<6;u++) {
      const opening=`${col(12+u)}${r}`, draw=`${col(4+u)}${r}`;
      formula(s,opening,r===16?`=${col(6+u)}6`:`=${col(24+u)}${r-1}`);
      formula(s,`${col(18+u)}${r}`,`=IF(AQ${r}="Ready",${opening}*$D$6*K${r}/$D$7,NA())`);
      const basis=`SUM(L${r}:Q${r},D${r}:I${r})`;
      formula(s,`${col(36+u)}${r}`,`=IF(AQ${r}="Ready",IF(${basis}=0,0,MIN(C${r},${basis})*(${opening}+${draw})/${basis}),NA())`);
      formula(s,`${col(30+u)}${r}`,`=IF(AQ${r}="Ready",IF(${basis}=0,IF(J${r}=0,0,NA()),J${r}*(${opening}+${draw})/${basis}),NA())`);
      formula(s,`${col(24+u)}${r}`,`=IF(AQ${r}="Ready",${opening}+${draw}-${col(36+u)}${r},NA())`);
    }
    formula(s,`AP${r}`,`=IF(AQ${r}="Ready",C${r}-SUM(AJ${r}:AO${r}),NA())`);
  }
  s.getRange('C16:AP51').setNumberFormat(money); s.getRange('K16:K51').setNumberFormat('0');
  flag(s,'AQ16:AQ51'); s.freezePanes.freezeRows(15); s.freezePanes.freezeColumns(3);
}
{
  const s=sheets.get('Rental');
  pair(s,5,'Gross cash rent',50000); pair(s,6,'GST assessed on rent',0);
  pair(s,7,'Opening funds held by agent',1000); pair(s,8,'Closing funds held by agent',3500); pair(s,9,'Other net cash receipts',0);
  pair(s,10,'GST cash paid / (refund)',0);
  result(s,11,'Net operating result','=IF(AND(COUNT(D5:D6)=2,MIN(D5:D6)>=0,D6<=D5,COUNT(C32:C34)=3,MIN(C32:C34)>=0),D5-D6-SUM(F17:F28)-SUM(C32:C34),NA())');
  result(s,12,'Cash before income tax','=IF(AND(ISNUMBER(D11),COUNT(D7:D10)=4,MIN(D7:D8)>=0,COUNT(D32:D34)=3,MIN(D32:D34)>=0),D5-SUM(C17:C28)-SUM(C32:D34)-(D8-D7)+D9-D10,NA())');
  result(s,13,'Ownership share status','=IF(AND(COUNTA(B39:B43)>0,COUNT(C39:C43)=COUNTA(B39:B43),MIN(C39:C43)>=0,ABS(SUM(C39:C43)-1)<0.00000001,COUNTIF(F39:F43,"Ready")=COUNTA(B39:B43)),"Ready","Check shares")','@');
  header(s,16,['Expense','Cash paid','Capital portion','Operating GST credit','Operating cost']);
  for(let r=17;r<=28;r++) {
    input(s,`B${r}`,['Agent costs','Repairs','Other costs'][r-17]??`Expense ${r-16}`,'@');
    input(s,`C${r}`,r===17?5000:r===18?3000:r===19?2000:0);
    input(s,`D${r}`,r===18?1000:0); input(s,`E${r}`,r===17?400:r===18?100:0);
    formula(s,`F${r}`,`=IF(AND(COUNT(C${r}:E${r})=3,MIN(C${r}:E${r})>=0,SUM(D${r}:E${r})<=C${r}),C${r}-SUM(D${r}:E${r}),NA())`);
  }
  header(s,31,['Loan','Interest cash paid','Principal cash paid']);
  for(let r=32;r<=34;r++){input(s,`B${r}`,`Loan ${r-31}`,'@');input(s,`C${r}`,r===32?4000:0);input(s,`D${r}`,r===32?6000:0);}
  header(s,38,['Owner','Ownership share','Operating allocation','Cash allocation','Input status']);
  for(let r=39;r<=43;r++) {
    input(s,`B${r}`,r<41?`Example owner ${r-38}`:'','@'); input(s,`C${r}`,r===39?0.6:r===40?0.4:null,pct);
    formula(s,`F${r}`,`=IF(B${r}="",IF(C${r}="","","Missing owner"),IF(AND(ISNUMBER(C${r}),C${r}>=0,SUMPRODUCT(--($B$39:$B$43=B${r}))=1),"Ready","Check shares"))`);
    formula(s,`D${r}`,`=IF(B${r}="","",IF($D$13="Ready",$D$11*C${r},NA()))`);
    formula(s,`E${r}`,`=IF(B${r}="","",IF($D$13="Ready",$D$12*C${r},NA()))`);
  }
  s.getRange('C17:F28').setNumberFormat(money);s.getRange('C32:D34').setNumberFormat(money);s.getRange('D39:E43').setNumberFormat(money);
  note(s,'H17','Capital portions include their GST.');
  note(s,'H18','Column E: GST credits on operating costs only.');
  note(s,'H20','The result excludes depreciation and tax adjustments.');
  note(s,'H21','Supply the GST and expense treatment.');
  note(s,'H23','Principal affects cash only.');
  note(s,'H24','Funds held bridge cash; they are not an expense.');
  note(s,'H26','GST settlements affect cash, not operating profit.');
  flag(s,'D13');flag(s,'F39:F43');
}
{
  const s=sheets.get('Packaging');
  pair(s,5,'Cash salary before packaging',100000); pair(s,6,'Assessed tax before packaging',25000);
  pair(s,7,'Employer on-costs before',20000); pair(s,8,'Assessed tax after packaging',22000); pair(s,9,'Employer on-costs after',18000);
  result(s,11,'Input status','=IF(AND(COUNT(D5:D9)=5,MIN(D5:D9)>=0,COUNT(C19:E23)=15,MIN(C19:E23)>=0,SUM(D19:D23)<=D5,D6<=D5,D8+SUM(E19:E23)<=D5-SUM(D19:D23)),"Ready","Check inputs")','@');
  result(s,12,'Employee cash before','=IF(D11="Ready",D5-D6,NA())');
  result(s,13,'Employee cash after','=IF(D11="Ready",D5-SUM(D19:D23)-D8-SUM(E19:E23),NA())');
  result(s,14,'Employer cost before','=IF(D11="Ready",D5+D7,NA())');
  result(s,15,'Employer cost after','=IF(D11="Ready",D5-SUM(D19:D23)+SUM(C19:C23)+D9-SUM(E19:E23),NA())');
  header(s,18,['Benefit','Employer cash cost','Pre-tax salary reduction','Post-tax contribution']);
  for(let r=19;r<=23;r++){input(s,`B${r}`,r===19?'Example lease':`Benefit ${r-18}`,'@');input(s,`C${r}`,r===19?10000:0);input(s,`D${r}`,r===19?10000:0);input(s,`E${r}`,r===19?2000:0);}
  note(s,'F5','Supply assessed tax and total on-costs for each arrangement. No tax or FBT rates are inferred.');
  note(s,'F7','Employer benefit costs must use one consistent GST basis.');
  note(s,'F9','Post-tax employee contributions are cash received by the employer.'); flag(s,'D11');
}
{
  const s=sheets.get('Amortisation');
  pair(s,5,'First annual period',2026,'0');
  note(s,'F5','Whole annual periods; no part-year apportionment. The reviewer supplies the life and write-off year.');
  header(s,8,['Cost cohort','Cost','Residual','Life in years','First period','Early write-off year',...Array.from({length:11},(_,i)=>2026+i),'Allocated in horizon','Closing book value','Input status']);
  for(let c=8;c<=18;c++)formula(s,`${col(c)}8`,`=$D$5+${c-8}`);
  for(let r=9;r<=19;r++) {
    input(s,`B${r}`,`Example cohort ${r-8}`,'@'); input(s,`C${r}`,r===9?5000:r===10?2000:r===11?9000:0);
    input(s,`D${r}`,0);input(s,`E${r}`,r===9?5:r===10?4:3,'0');input(s,`F${r}`,r===11?2028:2026,'0');input(s,`G${r}`,r===10?2028:0,'0');
    formula(s,`U${r}`,`=IF(AND(COUNT(C${r}:G${r})=5,MIN(C${r}:E${r})>=0,E${r}>0,E${r}=INT(E${r}),D${r}<=C${r},F${r}=INT(F${r}),G${r}=INT(G${r}),OR(G${r}=0,G${r}>=F${r}),COUNT($D$5)=1,$D$5=INT($D$5),F${r}>=$D$5,F${r}<=$D$5+10),"Ready","Check inputs")`);
    for(let c=8;c<=18;c++){
      const y=`${col(c)}$8`,prior=c===8?'0':`SUM(H${r}:${col(c-1)}${r})`;
      formula(s,`${col(c)}${r}`,`=IF($U${r}="Ready",IF(OR(${y}<$F${r},${y}>=$F${r}+$E${r},AND($G${r}>0,${y}>$G${r})),0,IF(AND($G${r}>0,${y}=$G${r}),$C${r}-$D${r}-${prior},($C${r}-$D${r})/$E${r})),NA())`);
    }
    formula(s,`S${r}`,`=SUM(H${r}:R${r})`);formula(s,`T${r}`,`=C${r}-S${r}`);
  }
  s.getRange('H9:T19').setNumberFormat(money);flag(s,'U9:U19');s.freezePanes.freezeRows(8);s.freezePanes.freezeColumns(2);
}
{
  const s=sheets.get('Timeline');
  pair(s,5,'Reporting window start',new Date(Date.UTC(2026,6,1)),'d-mmm-yyyy');
  pair(s,6,'Reporting window end',new Date(Date.UTC(2027,5,30)),'d-mmm-yyyy');
  header(s,9,['Interval','Start','End','Inclusive days','Days in window','Purpose / note','Evidence',...Array.from({length:24},(_,i)=>`Month ${i+1}`)]);
  for(let c=9;c<=32;c++){formula(s,`${col(c)}9`,`=EDATE(DATE(YEAR($D$5),MONTH($D$5),1),${c-9})`);s.getRange(`${col(c)}9`).setNumberFormat('mmm-yy');}
  for(let r=10;r<=21;r++) {
    input(s,`B${r}`,r===10?'Example occupation':r===11?'Example rental':'','@');
    input(s,`C${r}`,r===10?new Date(Date.UTC(2026,0,1)):r===11?new Date(Date.UTC(2026,9,1)):null,'d-mmm-yyyy');
    input(s,`D${r}`,r===10?new Date(Date.UTC(2026,8,30)):r===11?new Date(Date.UTC(2027,8,30)):null,'d-mmm-yyyy');
    input(s,`G${r}`,'','@');input(s,`H${r}`,'','@');
    const valid=`AND(COUNT(C${r}:D${r},$D$5:$D$6)=4,D${r}>=C${r},$D$6>=$D$5)`;
    formula(s,`E${r}`,`=IF(B${r}="","",IF(${valid},D${r}-C${r}+1,NA()))`);
    formula(s,`F${r}`,`=IF(B${r}="","",IF(${valid},MAX(0,MIN(D${r},$D$6)-MAX(C${r},$D$5)+1),NA()))`);
    for(let c=9;c<=32;c++)formula(s,`${col(c)}${r}`,`=IF($B${r}="","",IF(ISNUMBER($E${r}),IF(AND($C${r}<=EOMONTH(${col(c)}$9,0),$D${r}>=${col(c)}$9),1,""),NA()))`);
  }
  s.getRange('I9:AF21').format.columnWidth=9;
  s.getRange('I10:AF21').conditionalFormats.add('cellIs',{operator:'equal',formula:1,format:{fill:'#E8D9E2',font:{color:purple}}});
  note(s,'B24','Intervals and overlap use inclusive calendar days. Overlapping intervals are not automatically combined.');
  note(s,'B26','No residence exemption, CGT choice, estate obligation or statutory deadline is determined.');
  s.freezePanes.freezeRows(9);s.freezePanes.freezeColumns(2);
}
{
  const s=sheets.get('Deed register');pair(s,5,'Review date',new Date(Date.UTC(2026,9,18)),'d-mmm-yyyy');
  header(s,8,['Trust','Deed held','Signed','Stamped','Trustee','Appointor','Vesting date','Vesting review','FTE record','IEE record','NSW amendment record','Evidence']);
  for(let r=9;r<=28;r++) {
    for(let c=2;c<=13;c++)if(c!==9)input(s,`${col(c)}${r}`,'','@');
    if(r===9){input(s,`B${r}`,'Example trust','@');input(s,`H${r}`,new Date(Date.UTC(2030,9,18)),'d-mmm-yyyy');}
    else s.getRange(`H${r}`).setNumberFormat('d-mmm-yyyy');
    formula(s,`I${r}`,`=IF(B${r}="","",IF(AND(ISNUMBER(H${r}),ISNUMBER($D$5)),IF(H${r}<$D$5,"Past vesting date",IF(H${r}<=EDATE($D$5,60),"Within five years","Later")),"Missing date"))`);
  }
  s.getRange('I9:I28').format.columnWidth=22;
  flag(s,'I9:I28','Later');note(s,'B31','The five-year flag is a diary prompt. The deed and relevant law determine vesting and amendment requirements.');
  s.freezePanes.freezeRows(8);s.freezePanes.freezeColumns(2);
}
{
  const s=sheets.get('Carry forward');
  pair(s,5,'Eligibility supplied by reviewer','Yes','@');s.getRange('D5').dataValidation={rule:{type:'list',values:['Yes','No']}};
  pair(s,6,'Current annual capacity',30000);pair(s,7,'Already used this year',20000);pair(s,8,'Proposed additional use',25000);pair(s,9,'Current period label',2027,'0');
  result(s,11,'Input status','=IF(AND(OR(D5="Yes",D5="No"),COUNT(D6:D9)=4,MIN(D6:D8)>=0,D9=INT(D9),COUNT(B18:D22)=15,MIN(D18:D22)>=0,COUNTIF(H18:H22,"Ready")=5),"Ready","Check inputs")','@');
  result(s,12,'Current capacity left','=IF(D11="Ready",MAX(0,D6-D7),NA())');
  result(s,13,'Current capacity applied','=MIN(D8,D12)');
  result(s,14,'Proposal above all capacity','=IF(D11="Ready",MAX(0,D8-D13-SUM(F18:F22)),NA())');
  header(s,17,['Origin period','Last usable period','Remaining bucket','Available now','Applied to proposal','Usable after','Input status']);
  for(let r=18;r<=22;r++) {
    input(s,`B${r}`,2022+r-18,'0');input(s,`C${r}`,2027+r-18,'0');input(s,`D${r}`,5000);
    formula(s,`H${r}`,`=IF(AND(COUNT(B${r}:D${r})=3,B${r}=INT(B${r}),C${r}=INT(C${r}),C${r}>=B${r},D${r}>=0${r>18?`,B${r}>B${r-1},C${r}>=C${r-1}`:''}),"Ready","Check buckets")`);
    formula(s,`E${r}`,`=IF($D$11="Ready",IF(AND($D$5="Yes",C${r}>=$D$9,B${r}<$D$9),D${r},0),NA())`);
    formula(s,`F${r}`,`=MIN(E${r},MAX(0,$D$8-$D$13-${r===18?'0':`SUM($F$18:F${r-1})`}))`);
    formula(s,`G${r}`,`=E${r}-F${r}`);
  }
  s.getRange('D18:G22').setNumberFormat(money);flag(s,'D11');flag(s,'H18:H22');
  s.getRange('H18:H22').format.horizontalAlignment = 'center';
  note(s,'B25','Enter remaining buckets after all earlier use. Rows must be oldest first, with nondecreasing expiry.');
  note(s,'B27','Examples are arbitrary capacity figures. This sheet does not determine super eligibility, caps or deductibility.');
}

{
  const s=sheets.get('Loan offers');
  pair(s,5,'Annual rate shock',0.02,pct);
  note(s,'F5','Nominal annual rates, monthly compounding, end-of-month payments. Fees are paid in cash.');
  note(s,'B7','Compare offers with the same principal and term. Total outlay includes regular payments, balloon and fees.');
  header(s,9,['Offer','Principal','Annual rate','Months','Final balloon','Upfront fee','Monthly fee','Loan payment','Total interest','Total fees','Total outlay','Stressed payment','Input status']);
  for(let r=10;r<=17;r++) {
    input(s,`B${r}`,r<12?`Example ${r-9}`:'','@');
    input(s,`C${r}`,r<12?100000:0); input(s,`D${r}`,r===10?0.06:0.065,pct);
    input(s,`E${r}`,60,'0');input(s,`F${r}`,0);input(s,`G${r}`,r===10?500:0);input(s,`H${r}`,r===10?10:0);
    formula(s,`N${r}`,`=IF(B${r}="","",IF(AND(COUNT(C${r}:H${r})=6,C${r}>0,MIN(D${r}:H${r})>=0,E${r}>0,E${r}=INT(E${r}),F${r}<=C${r},ISNUMBER($D$5),D${r}+$D$5>=0,SUMPRODUCT(--($B$10:$B$17=B${r}))=1),"Ready","Check inputs"))`);
    formula(s,`I${r}`,`=IF(B${r}="","",IF(N${r}="Ready",IF(D${r}=0,(C${r}-F${r})/E${r},-PMT(D${r}/12,E${r},C${r},-F${r})),NA()))`);
    formula(s,`J${r}`,`=IF(B${r}="","",I${r}*E${r}+F${r}-C${r})`);
    formula(s,`K${r}`,`=IF(B${r}="","",G${r}+H${r}*E${r})`);
    formula(s,`L${r}`,`=IF(B${r}="","",I${r}*E${r}+F${r}+K${r})`);
    formula(s,`M${r}`,`=IF(B${r}="","",IF(N${r}="Ready",IF(D${r}+$D$5=0,(C${r}-F${r})/E${r},-PMT((D${r}+$D$5)/12,E${r},C${r},-F${r}))+H${r},NA()))`);
  }
  s.getRange('I10:M17').setNumberFormat(money); flag(s,'N10:N17');
  note(s,'B20','Loan payment excludes fees; stressed payment includes the monthly fee. Fixed-rate assumptions do not forecast a lender.');
  note(s,'B22','No extra payments, rate resets, offset account, break costs or borrowing-capacity decision.');
}
{
  const s=sheets.get('Break even');
  [['Fixed costs per period',60000],['Selling price per unit',100],['Variable cost per unit',40],
    ['Expected units sold',1500],['Target profit per period',30000],['Maximum units per period',1800]].forEach(([label,value],i)=>pair(s,i+5,label,value,i>=3?'0.00':money));
  result(s,12,'Input status','=IF(AND(COUNT(D5:D10)=6,MIN(D5:D10)>=0,D6>D7),"Ready","Check costs or inputs")','@');
  result(s,13,'Contribution per unit','=IF(D12="Ready",D6-D7,NA())');
  result(s,14,'Break-even units','=IF(D12="Ready",ROUNDUP(D5/D13,0),NA())','0');
  result(s,15,'Units for target profit','=IF(D12="Ready",ROUNDUP((D5+D9)/D13,0),NA())','0');
  result(s,16,'Expected profit','=IF(D12="Ready",D8*D13-D5,NA())');
  result(s,17,'Units above break even','=IF(D12="Ready",D8-D14,NA())','0.00');
  result(s,18,'Target capacity status','=IF(D12<>"Ready","Check inputs",IF(D15>D10,"Above capacity","Within capacity"))','@');
  result(s,19,'Price for target at capacity','=IF(D12="Ready",IF(D10>0,(D5+D9)/D10+D7,NA()),NA())');
  note(s,'F5','One common period for costs, volume, capacity and target profit.');
  note(s,'F7','Price and variable cost are constant. Supply amounts on a consistent GST basis.');
  note(s,'F9','Whole break-even/target units round upwards; other stored values remain unrounded.');
  note(s,'F11','Losses and capacity shortfalls remain visible. This model excludes financing and income tax.');
  flag(s,'D12');flag(s,'D18','Within capacity');
}
{
  const s=sheets.get('Working capital');
  [['Credit sales for period',1200000],['Cost of sales for period',600000],['Credit purchases for period',720000],
    ['Days in reporting period',365]].forEach(([label,value],i)=>pair(s,i+5,label,value,i===3?'0':money));
  result(s,10,'Input status','=IF(AND(COUNT(D5:D8)=4,MIN(D5:D7)>=0,D8>0,COUNT(C15:D17)=6,MIN(C15:D17)>=0),"Ready","Check inputs")','@');
  result(s,11,'Cash released / (invested)','=IF(D10="Ready",SUM(G15:G17),NA())');
  header(s,14,['Balance','Current days','Target days','Current balance','Target balance','Cash released']);
  const items=[['Receivables',45,30,5],['Inventory',60,50,6],['Payables',30,45,7]];
  items.forEach(([label,current,target,source],i)=>{
    const r=15+i;cell(s,`B${r}`,label);input(s,`C${r}`,current,'0.00');input(s,`D${r}`,target,'0.00');
    formula(s,`E${r}`,`=IF($D$10="Ready",$D$${source}/$D$8*C${r},NA())`);
    formula(s,`F${r}`,`=IF($D$10="Ready",$D$${source}/$D$8*D${r},NA())`);
    formula(s,`G${r}`,i===2?`=F${r}-E${r}`:`=E${r}-F${r}`);
  });
  s.getRange('E15:G17').setNumberFormat(money);
  note(s,'B20','Use credit sales for receivables, cost of sales for inventory and credit purchases for payables.');
  note(s,'B22','Keep balance and flow valuations consistent, including GST. Amounts are steady-state estimates, not a dated cash forecast.');
  note(s,'B24','Negative cash release means more cash is tied up. Supplier terms and feasible targets require business review.');flag(s,'D10');
}

for(const name of names.filter(n=>n!=='Read me')) {
  const s=sheets.get(name);
  s.tabColor=['Timeline','Deed register'].includes(name)?'#000000':name==='Allocation'?'#990024':purple;
  cell(s,'B1','Contents');
  s.getRange('B1').format.font={name:'Arial',size:10,color:purple};
}

book.recalculate();
const checks = await book.inspect({kind:'match',searchTerm:'#REF!|#DIV/0!|#VALUE!|#NAME\\?|#N/A|#NUM!|#NULL!|#SPILL!|#CALC!|not implemented',options:{useRegex:true,maxResults:30},summary:'Example formula errors'});
console.log(checks.ndjson);
if(checks.ndjson.split('\n').filter(Boolean).some(line=>JSON.parse(line).kind==='match')) throw new Error('Example calculation errors; export cancelled');
console.log((await book.inspect({kind:'table',range:'Contractor!B14:D19',include:'values,formulas',tableMaxRows:6,tableMaxCols:3})).ndjson);
// The output is fixed relative to this script, with no external path input.
// nosemgrep: javascript_pathtraversal_rule-non-literal-fs-filename
await fs.mkdir(path.dirname(output),{recursive:true});
await (await SpreadsheetFile.exportXlsx(book)).save(output);
console.log(`Saved ${output}`);
if(previewDir) {
  // OZZIT_PREVIEW_DIR is the author's explicit local destination. Filenames below
  // come only from the fixed sheet list and literal labels, never workbook input.
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  await fs.mkdir(previewDir,{recursive:true}); // nosemgrep: javascript_pathtraversal_rule-non-literal-fs-filename
  for(const name of names) {
    const range=name==='Allocation'?'B1:T19':name==='Mixed loan'?'B1:K20':name==='Amortisation'?'B1:K14':name==='Timeline'?'B1:O15':name==='Deed register'?'B1:N14':name==='Read me'?'B2:F33':name==='Vendor finance'?'B1:N26':name==='Rental'?'B1:N43':name==='Loan offers'?'B1:N23':'B1:J28';
    const png=await book.render({sheetName:name,range,scale:1.4,format:'png'});
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.writeFile(path.join(previewDir,`${name.replaceAll(' ','-')}.png`),new Uint8Array(await png.arrayBuffer())); // nosemgrep: javascript_pathtraversal_rule-non-literal-fs-filename
  }
  for(const [name,range,label] of [['Mixed loan','L15:AC20','mixed-balances'],['Mixed loan','AD15:AQ20','mixed-allocations'],['Amortisation','L8:U14','amortisation-closing']]) {
    const png=await book.render({sheetName:name,range,scale:1.4,format:'png'});
    // eslint-disable-next-line security/detect-non-literal-fs-filename
    await fs.writeFile(path.join(previewDir,`${label}.png`),new Uint8Array(await png.arrayBuffer())); // nosemgrep: javascript_pathtraversal_rule-non-literal-fs-filename
  }
  console.log(`Rendered ${names.length} sheet previews`);
}
