# End-to-end check of ozzit.xlsx in a real Excel calculation engine.
#
#   powershell -ExecutionPolicy Bypass -File tools\excel_selftest.ps1 [path\to\ozzit.xlsx]
#
# Requires Excel with LAMBDA support (Microsoft 365, or Excel 2024 and later). GitHub's
# runners have no Excel, so CI runs tools/verify_workbook.py instead and this stays a
# local gate: verify_workbook.py checks the file's structure, this checks its arithmetic.
#
# The workbook is opened, fully recalculated, scanned for error cells, probed with a
# temporary sheet of assertions, then closed WITHOUT saving. It is never modified.

param([string]$Path = "$PSScriptRoot\..\ozzit.xlsx")

$ErrorActionPreference = 'Stop'
$L = [char]0x03BB                 # this file stays pure ASCII so encoding cannot corrupt it
$Path = (Resolve-Path $Path).Path

$checks = @()
function Check($id, $formula) { $script:checks += @{ id = $id; f = $formula } }

# Each formula must evaluate to exactly "OK". Anything else is printed as the failure.
function Near($id, $expr, $want, $tol = '0.000001') {
    Check $id "=LET(v, $expr, IF(ISERROR(v), `"ERROR`", IF(ABS(v-($want))<$tol, `"OK`", `"got `"&TEXT(v,`"0.00####`"))))"
}
function Same($id, $expr, $want) {
    Check $id "=LET(v, $expr, IF(ISERROR(v), `"ERROR`", IF(v=`"$want`", `"OK`", `"got [`"&v&`"]`")))"
}

# Every function's help, and every worked example the help prints that stands on its
# own, generated from src/ by tools/generate_selftest_examples.py. A help change means
# rerunning the generator; tools/tests fails when the fragment is stale.
. (Join-Path $PSScriptRoot 'selftest_examples.ps1')

$dv = "oz.DiminishingValue$L"
$pc = "oz.PrimeCost$L"
$ga = "oz.GSTAdd$L"
$ge = "oz.GSTExtract$L"
$fy = "oz.FinancialYear$L"

# --- Depreciation: a schedule must always sum to cost, whatever the effective life.
# ATO effective lives are frequently fractional (3 1/3, 6 2/3, 13 1/3), and a life of
# 2 years or less drives the diminishing-value rate to its 100% cap.
foreach ($life in '1', '1.5', '2', '2.5', '3', '10/3', '4', '5', '20/3', '8', '10', '40/3', '15', '20', '25', '40') {
    $tag = $life.Replace('/', 'over')
    Near "DV sums to cost, life $life"  "SUM($dv(1000,$life))"  '1000'
    Near "PC sums to cost, life $life"  "SUM($pc(1000,$life))"  '1000'
    Near "DV period count, life $life"  "COLUMNS($dv(1000,$life))"  "MAX(1,ROUNDUP($life,0))"
    Near "PC period count, life $life"  "COLUMNS($pc(1000,$life))"  "MAX(1,ROUNDUP($life,0))"
    Near "DV never negative, life $life" "SUMPRODUCT(--($dv(1000,$life)<0))" '0'
    Near "PC never negative, life $life" "SUMPRODUCT(--($pc(1000,$life)<0))" '0'
}
# The worked examples printed in each function's own inline help.
Same 'DV documented example' "TEXTJOIN(`",`",FALSE,TEXT($dv(1000,5),`"0.00`"))" '400.00,240.00,144.00,86.40,129.60'
Same 'PC documented example' "TEXTJOIN(`",`",FALSE,TEXT($pc(1000,5),`"0.00`"))" '200.00,200.00,200.00,200.00,200.00'
# Diminishing value must fall period on period. The final period is excluded because it
# carries the residual write-off, which is deliberately larger than the period before it.
Near 'DV declines over time' "SUMPRODUCT(--(INDEX($dv(1000,10),1,SEQUENCE(,8))<INDEX($dv(1000,10),1,SEQUENCE(,8)+1)))" '0'
Near 'DV residual is the tail' "INDEX($dv(1000,5),1,5)-129.6" '0'
Same 'DV help with no args'  "INDEX($dv(),1,1)" 'FUNCTION:'
Same 'PC help with no args'  "INDEX($pc(),1,1)" 'FUNCTION:'

# --- GST
Near 'GST add, default rate'      "$ga(100)"          '110'
Near 'GST add, blank rate cell'   "$ga(100,Z1)"       '110'
Near 'GST add, explicit zero'     "$ga(100,0)"        '100'
Near 'GST add, explicit 15%'      "$ga(100,0.15)"     '115'
Near 'GST add, array'             "SUM($ga({100;250}))" '385'
Near 'GST extract, default rate'  "$ge(110)"          '10'
Near 'GST extract, blank rate'    "$ge(110,Z1)"       '10'
Near 'GST extract, explicit zero' "$ge(110,0)"        '0'
Near 'GST extract, array'         "SUM($ge({110;220}))" '30'
Near 'GST round trip'             "$ge($ga(100))"     '10'
Same 'GST add keeps blanks blank'     "`"[`"&$ga(Z1)&`"]`"" '[]'
Same 'GST extract keeps blanks blank' "`"[`"&$ge(Z1)&`"]`"" '[]'
Same 'GST add help with no args'      "INDEX($ga(),1,1)" 'FUNCTION:'
Same 'GST extract help with no args'  "INDEX($ge(),1,1)" 'FUNCTION:'

# --- Financial year. The headline use is labelling a COLUMN of dates, so the array
# path matters more than the scalar one.
Same 'FY 30 Jun 2026'          "$fy(DATE(2026,6,30))"  'FY2026'
Same 'FY 1 Jul 2026'           "$fy(DATE(2026,7,1))"   'FY2027'
Same 'FY 31 Dec 2026'          "$fy(DATE(2026,12,31))" 'FY2027'
Same 'FY 29 Feb 2028'          "$fy(DATE(2028,2,29))"  'FY2028'
Same 'FY blank cell'           "`"[`"&$fy(Z1)&`"]`""    '[]'
Same 'FY array spans boundary' "TEXTJOIN(`"|`",FALSE,$fy(DATE(2026,6,30)+{0;1;2}))" 'FY2026|FY2027|FY2027'
Same 'FY array, whole year'    "TEXTJOIN(`"|`",FALSE,$fy(DATE(2026,1,1)+{0;200;400}))" 'FY2026|FY2027|FY2027'
Same 'FY January start'        "$fy(DATE(2026,7,1),1)"  'FY2026'
Same 'FY January start, Dec'   "$fy(DATE(2026,12,31),1)" 'FY2026'
Same 'FY April start, 31 Mar'  "$fy(DATE(2026,3,31),4)" 'FY2026'
Same 'FY April start, 1 Apr'   "$fy(DATE(2026,4,1),4)"  'FY2027'
Same 'FY December start'       "$fy(DATE(2026,12,1),12)" 'FY2027'
Same 'FY array, January start' "TEXTJOIN(`"|`",FALSE,$fy(DATE(2026,6,30)+{0;1},1))" 'FY2026|FY2026'
Same 'FY help with no args'    "INDEX($fy(),1,1)" 'FUNCTION:'

# --- the Australian tax worksheet, as the reader actually sees it
$au = "'Australian tax'!"
Near 'Sheet: DV total equals cost' "${au}B10-${au}A6" '0'
Near 'Sheet: PC total equals cost' "${au}B11-${au}A6" '0'
Near 'Sheet: GST added'            "${au}B16" '1100'
Near 'Sheet: GST extracted'        "${au}B19" '100'
Same 'Sheet: FY 30 Jun 2026'       "${au}B23" 'FY2026'
Same 'Sheet: FY 1 Jul 2026'        "${au}B24" 'FY2027'
Same 'Sheet: FY 15 Aug 2026'       "${au}B25" 'FY2027'
Same 'Sheet: FY 31 Dec 2026'       "${au}B26" 'FY2027'

# --- Debt sculpting. The debt module has never had a numeric check of any kind, which is
# how DebtSculptVariableLRV shipped from v1.2.0 to v2.2.0 adding each period's interest
# back into a balance the same period's cash had already paid. These are balance
# identities rather than expected figures: a schedule that satisfies all of them cannot
# be double-counting, whatever the inputs.
$lrv = "oz.DebtSculptVariableLRV$L"
$dsf = "oz.DebtSculptFixed$L"
$dsv = "oz.DebtSculptVariable$L"
$ilrv = "oz.InterestLRV$L"

# 1,000 drawn in period 1, 300 of cash a period, 1.2 times covered, 6% a year, 5 years.
# Rows are opening balance, interest, MINUS the principal repayment, closing balance.
$sched = "$lrv(, {1000,0,0,0,0}, {300,300,300,300,300}, {1.2,1.2,1.2,1.2,1.2}, {0.06,0.06,0.06,0.06,0.06}, 12)"

Near 'Debt: repayments retire the principal exactly' "SUM(INDEX($sched, 3, 0))" '-1000' '0.0000001'
Near 'Debt: schedule ends at zero'                   "INDEX($sched, 4, 5)"      '0'     '0.0000001'
Near 'Debt: closing = opening less repayment' `
     "SUMPRODUCT(ABS(INDEX($sched,4,0) - INDEX($sched,1,0) - INDEX($sched,3,0)))" '0' '0.0000001'
# No new debt after period 1, so every opening must be the previous closing. Line the two
# rows up by dropping the first opening and the last closing rather than by position.
Near 'Debt: each opening = the last closing' `
     "SUMPRODUCT(ABS(DROP(INDEX($sched,1,0),,1) - DROP(INDEX($sched,4,0),,-1)))" '0' '0.0000001'
Near 'Debt: cash used never exceeds CFADS/DSCR' `
     "MAX(INDEX($sched,2,0) - INDEX($sched,3,0)) - 250" '0' '0.0000001'
Near 'Debt: balance never goes negative'             "MIN(0, MIN(INDEX($sched, 4, 0)))" '0' '0.0000001'

# Cash well over the debt must clear it in one period, not leave twice the interest behind.
Near 'Debt: surplus cash clears the balance' `
     "INDEX($lrv(, {1000,0}, {1800,1800}, {1.2,1.2}, {0.06,0.06}, 12), 4, 1)" '0' '0.0000001'
# No cash must capitalise one period of interest, not two.
Near 'Debt: no cash capitalises interest once' `
     "LET(s, $lrv(, {1000,0}, {0,0}, {1.2,1.2}, {0.06,0.06}, 12), INDEX(s,4,1) - 1000 - INDEX(s,2,1))" '0' '0.0000001'

# The other two sculpting functions pay the whole debt service, so their balances differ,
# but the same roll-forward has to hold: closing = opening + interest - debt service.
foreach ($fn in $dsf, $dsv) {
    $arg = if ($fn -eq $dsf) { '1.2, 0.06' } else { '{1.2,1.2,1.2}, {0.06,0.06,0.06}' }
    $s2 = "$fn(, {1000,0,0}, {300,300,300}, $arg, 12)"
    Near "Debt: roll-forward holds for $fn" `
         "SUMPRODUCT(ABS(INDEX($s2,4,0) - INDEX($s2,1,0) - INDEX($s2,2,0) - INDEX($s2,3,0)))" '0' '0.0000001'
}

# The row a reader is told to label. Only the LRV function reports a principal repayment.
# Searched rather than indexed, so a help row added above the list does not move it.
Near 'Debt: LRV names a principal repayment row' "SUMPRODUCT(--(INDEX($lrv(),0,2)=`"Principal repayments`"))" '1'
Near 'Debt: fixed names a debt service row'     "SUMPRODUCT(--(INDEX($dsf(),0,2)=`"Debt service (interest and principal)`"))" '1'
Near 'Debt: variable names a debt service row'  "SUMPRODUCT(--(INDEX($dsv(),0,2)=`"Debt service (interest and principal)`"))" '1'

# The debt module's one worked example.
Near 'Debt: InterestLRV worked example' "$ilrv(6666.37, 3.50, 90000, 0.03/12)" '222.90' '0.005'

# --- PeriodStart. A period anchored on a month end is defined by EDATE: the anchor's day
# of the month where the target month has one, the month's own end where it does not, so
# 31 January monthly runs 31 Jan, 28 Feb, 31 Mar. Up to v2.3.0 the function walked the
# calendar and took a single day back off whatever overflowed, which put 5 March in a
# period starting 2 March. Each grid check compares 180 dates at 13-day steps against
# that EDATE schedule, which is stated here rather than borrowed from the function.
$ps = "oz.PeriodStart$L"
$psWant = 'MAP(ds, LAMBDA(d, LET(s, EDATE(a, SEQUENCE(121,1,-60) * m), MAX(IF(s <= d, s, 0)))))'

foreach ($day in '31', '30', '29', '28', '15', '1') {
    foreach ($m in '1', '3', '12') {
        Near "PeriodStart: anchor 2026-01-$day, $m-month periods" `
             ("LET(a, DATE(2026,1,$day), m, $m, ds, DATE(2024,1,1) + SEQUENCE(1,180,0,13), " +
              "SUMPRODUCT(--(MAP(ds, LAMBDA(d, $ps(a, m, d))) <> $psWant)))") '0'
    }
}
Near 'PeriodStart: anchor 29 Feb 2028, monthly' `
     ("LET(a, DATE(2028,2,29), m, 1, ds, DATE(2024,1,1) + SEQUENCE(1,180,0,13), " +
      "SUMPRODUCT(--(MAP(ds, LAMBDA(d, $ps(a, m, d))) <> $psWant)))") '0'

# The reported case, its neighbours on either side, and the documented example.
Same 'PeriodStart: 31 Jan monthly, 5 Mar' `
     "TEXT($ps(`"31/1/2026`", 1, `"5/3/2026`"), `"yyyy-mm-dd`")" '2026-02-28'
Same 'PeriodStart: 31 Jan monthly, 27 Feb' `
     "TEXT($ps(`"31/1/2026`", 1, `"27/2/2026`"), `"yyyy-mm-dd`")" '2026-01-31'
Same 'PeriodStart: 31 Jan monthly, 28 Feb' `
     "TEXT($ps(`"31/1/2026`", 1, `"28/2/2026`"), `"yyyy-mm-dd`")" '2026-02-28'
Same 'PeriodStart: 31 Jan monthly, 31 Mar' `
     "TEXT($ps(`"31/1/2026`", 1, `"31/3/2026`"), `"yyyy-mm-dd`")" '2026-03-31'
Same 'PeriodStart: 31 Dec monthly, 29 Feb 2028' `
     "TEXT($ps(`"31/12/2025`", 1, `"29/2/2028`"), `"yyyy-mm-dd`")" '2028-02-29'
Same 'PeriodStart: 30 Nov monthly, 15 Feb' `
     "TEXT($ps(`"30/11/2025`", 1, `"15/2/2026`"), `"yyyy-mm-dd`")" '2026-01-30'
Same 'PeriodStart: documented example' `
     "TEXT($ps(`"2026-01-01`", 3, `"2026-04-15`"), `"yyyy-mm-dd`")" '2026-04-01'
Same 'PeriodStart: date before the anchor' `
     "TEXT($ps(DATE(2026,1,1), 3, DATE(2025,11,15)), `"yyyy-mm-dd`")" '2025-10-01'
Same 'PeriodStart: date on the anchor' `
     "TEXT($ps(DATE(2026,1,15), 3, DATE(2026,1,15)), `"yyyy-mm-dd`")" '2026-01-15'
Same 'PeriodStart: help with no args' "INDEX($ps(),1,1)" 'FUNCTION:'

# --- TimelineOffset. The interval is read off the timeline's first two dates and, up to
# v2.3.0, converted to whole months and divided by. A daily, weekly or fortnightly
# timeline rounds to no months at all, so every one of them returned #DIV/0!.
$to = "oz.TimelineOffset$L"
$daily = 'DATE(2026,1,1) + SEQUENCE(1,60,0,1)'
$weekly = 'DATE(2026,1,1) + SEQUENCE(1,60,0,7)'
$fortnightly = 'DATE(2026,1,1) + SEQUENCE(1,60,0,14)'

Near 'TimelineOffset: daily, 10 days in'        "$to(DATE(2026,1,11), $daily)"        '10'
Near 'TimelineOffset: daily, 3 days before'     "$to(DATE(2025,12,29), $daily)"       '-3'
Near 'TimelineOffset: weekly, 20 days in'       "$to(DATE(2026,1,21), $weekly)"       '2'
Near 'TimelineOffset: weekly, 3 days before'    "$to(DATE(2025,12,29), $weekly)"      '-1'
Near 'TimelineOffset: weekly, one whole period before' "$to(DATE(2025,12,25), $weekly)" '-1'
Near 'TimelineOffset: fortnightly, 30 days in'  "$to(DATE(2026,1,31), $fortnightly)"  '2'

# A sub-monthly period is a fixed number of days, so the offset is the day difference
# floored by that count. 200 dates at 3-day steps, starting three months before the
# timeline does, so the negative side is covered too.
foreach ($tl in @('daily', $daily, '1'), @('weekly', $weekly, '7'), @('fortnightly', $fortnightly, '14')) {
    Near "TimelineOffset: $($tl[0]) counts whole periods" `
         ("LET(t, $($tl[1]), ds, DATE(2025,10,1) + SEQUENCE(1,200,0,3), " +
          "SUMPRODUCT(--(MAP(ds, LAMBDA(d, $to(d, t))) <> " +
          "MAP(ds, LAMBDA(d, INT((d - INDEX(t,1)) / $($tl[2])))))))") '0'
}

# and the month path must answer exactly what it always did, month ends included.
foreach ($tl in @('monthly', '1', '15'), @('quarterly', '3', '15'), @('yearly', '12', '15'),
                @('monthly off a 31st', '1', '31')) {
    Near "TimelineOffset: $($tl[0]) unchanged" `
         ("LET(b, DATE(2026,1,$($tl[2])), mpp, $($tl[1]), t, EDATE(b, SEQUENCE(1,40,0,mpp)), " +
          "ds, DATE(2024,1,1) + SEQUENCE(1,200,0,11), " +
          "SUMPRODUCT(--(MAP(ds, LAMBDA(d, $to(d, t))) <> " +
          "MAP(ds, LAMBDA(d, LET(ks, SEQUENCE(121,1,-60), s, EDATE(b, ks * mpp), " +
          "MAX(IF(s <= d, ks, -9999))))))))") '0'
}
Same 'TimelineOffset: help with no args' "INDEX($to(),1,1)" 'FUNCTION:'

# The two worked examples printed in the function's own help, which a reader is meant to
# copy. Neither could be run as printed: the call was missing its two closing brackets.
Near 'TimelineOffset: documented example, inside the timeline' `
     "$to(`"15/2/2026`", EDATE(`"1/1/2026`", SEQUENCE( , 12, 0)))" '1'
Near 'TimelineOffset: documented example, before the timeline' `
     "$to(`"15/2/2025`", EDATE(`"1/1/2026`", SEQUENCE( , 12, 0)))" '-11'


# --- Amortise on a timeline shorter than a month. The period length is read off the first
# two dates and rounded to whole months, which is nought below about a fortnight, and the
# next line divided by it: every daily, weekly and fortnightly call came back #DIV/0!. The
# schedule is still solved monthly, so the test is that the same money turns up, dated into
# the period that holds each month's start, and that the periods between hold nothing.
$am = "oz.Amortise$L"
$amMo = "$am(10000, 0.05, 12, DATE(2026,1,1), EDATE(DATE(2026,1,1), SEQUENCE( , 14, 0)))"
$amWk = "$am(10000, 0.05, 12, DATE(2026,1,1), DATE(2026,1,1) + SEQUENCE( , 52, 0) * 7)"
$amFn = "$am(10000, 0.05, 12, DATE(2026,1,1), DATE(2026,1,1) + SEQUENCE( , 26, 0) * 14)"
$amDy = "$am(10000, 0.05, 12, DATE(2026,1,1), DATE(2026,1,1) + SEQUENCE( , 400, 0))"

Near 'Amortise: weekly is a schedule, not an error'      "SUMPRODUCT(--ISERROR($amWk))" '0'
Near 'Amortise: fortnightly is a schedule, not an error' "SUMPRODUCT(--ISERROR($amFn))" '0'
Near 'Amortise: daily is a schedule, not an error'       "SUMPRODUCT(--ISERROR($amDy))" '0'
# One sample of every stock and one instance of every flow, whatever the period length.
Near 'Amortise: weekly holds the same money as monthly'      "SUM($amWk) - SUM($amMo)" '0' '0.0000001'
Near 'Amortise: fortnightly holds the same money as monthly' "SUM($amFn) - SUM($amMo)" '0' '0.0000001'
Near 'Amortise: daily holds the same money as monthly'       "SUM($amDy) - SUM($amMo)" '0' '0.0000001'
Near 'Amortise: weekly fills no extra periods' `
     "SUMPRODUCT(--(INDEX($amWk,3,0)<>0)) - SUMPRODUCT(--(INDEX($amMo,3,0)<>0))" '0'
Near 'Amortise: weekly draws the debt down once' "SUMPRODUCT(--(INDEX($amWk,1,0)<>0)) - 1" '0'
# Both timelines open on 1 January 2026, so month one is period one on each. Month two opens
# on 1 February, 31 days on, which is the fifth week and not the second. These read row 3,
# the interest, and row 2, the balance: both fall month on month, so a figure in the wrong
# period is a figure that does not match. The payment row would not do, because a level
# payment is the same number every month and would match wherever it landed.
Near 'Amortise: weekly, month one interest is period one'  "INDEX($amWk,3,1) - INDEX($amMo,3,1)" '0' '0.0000001'
Near 'Amortise: weekly, month two interest is the week holding 1 February' `
     "INDEX($amWk,3,5) - INDEX($amMo,3,2)" '0' '0.0000001'
Near 'Amortise: weekly, month three interest is the week holding 1 March' `
     "INDEX($amWk,3,9) - INDEX($amMo,3,3)" '0' '0.0000001'
Near 'Amortise: weekly, month two balance is the week holding 1 February' `
     "INDEX($amWk,2,5) - INDEX($amMo,2,2)" '0' '0.0000001'
Near 'Amortise: weekly, the weeks between hold no interest' `
     "SUMPRODUCT(ABS(INDEX($amWk,3,SEQUENCE(,3,2))))" '0' '0.0000001'
Near 'Amortise: weekly, the weeks between hold no payment' `
     "SUMPRODUCT(ABS(INDEX($amWk,4,SEQUENCE(,3,2))))" '0' '0.0000001'
# A twenty-day period is no whole number of months either, and used to round to one and be
# laid out as though it were a month long.
Near 'Amortise: twenty-day periods hold the same money as monthly' `
     "SUM($am(10000, 0.05, 12, DATE(2026,1,1), DATE(2026,1,1) + SEQUENCE( , 20, 0) * 20)) - SUM($amMo)" '0' '0.0000001'
# An unevenly spaced sub-monthly timeline must still tile the calendar: no month counted
# twice, none dropped. This one alternates 5 and 40 day periods.
$uneven = "DATE(2026,1,1) + SCAN(0, SEQUENCE( , 27, 0), LAMBDA(a,k, IF(k = 0, 0, a + IF(MOD(k,2) = 1, 5, 20))))"
Near 'Amortise: an uneven timeline still counts each month once' `
     "SUM($am(10000, 0.05, 12, DATE(2026,1,1), $uneven)) - SUM($amMo)" '0' '0.0000001'
# The month path is untouched. These three totals are what v2.5.0 produced.
$amL = "10000, 0.05, 48, DATE(2026,1,1)"
Near 'Amortise: monthly unchanged'    "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 24, 0))))"     '377875.31' '0.005'
Near 'Amortise: quarterly unchanged'  "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 3)))" '132616.32' '0.005'
Near 'Amortise: six-monthly unchanged' "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 6)))" '92617.78' '0.005'
Same 'Amortise: help with no args' "INDEX($am(),1,1)" 'FUNCTION:'

# --- Depreciate. Every period but the last takes its end date from the next period's start.
# The last had EDATE(its own start, months per period) - 1, which on a sub-monthly timeline
# is the day BEFORE it opens, so the final period collected nothing: 48 weekly periods from
# 1 January 2026 dropped December and reported 1,833.37 of a 2,000.00 year.
$dp = "oz.Depreciate$L"
$dpA = "10000, DATE(2026,1,1), 5"
$dpMo = "$dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 12, 0)))"
$dpW48 = "$dp($dpA, DATE(2026,1,1) + SEQUENCE( , 48, 0) * 7)"
$dpW49 = "$dp($dpA, DATE(2026,1,1) + SEQUENCE( , 49, 0) * 7)"
Near 'Depreciate: 48 weekly periods depreciate a full year' "SUM(INDEX($dpW48,3,0)) - 2000" '0' '0.005'
Near 'Depreciate: 49 weekly periods agree with 48' `
     "SUM(INDEX($dpW49,3,0)) - SUM(INDEX($dpW48,3,0))" '0' '0.005'
Near 'Depreciate: weekly agrees with monthly' `
     "SUM(INDEX($dpW48,3,0)) - SUM(INDEX($dpMo,3,0))" '0' '0.005'
Near 'Depreciate: no period is counted twice' "SUMPRODUCT(--(INDEX($dpW48,3,0)<>0)) - 12" '0'
# Whole-month intervals are unchanged. Two, four and six months never failed: the SWITCH
# lookups that only listed 1, 3 and 12 were read by two bindings nothing else read, so
# Excel never evaluated them. They are gone rather than generalised.
# Summing the whole block cannot see the depreciation row: BookValue is OpeningAmount less
# Depreciation on the same period, so the two cancel cell for cell and the total collapses to
# the CAPEX plus twice the opening balances. Each interval is pinned twice, once on the
# depreciation row and once on the block, so neither a changed schedule nor a changed
# balance can pass unnoticed.
Near 'Depreciate: monthly depreciation unchanged'     "SUM(INDEX($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 24, 0))),3,0))"     '4000.00' '0.005'
Near 'Depreciate: quarterly depreciation unchanged'   "SUM(INDEX($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 3)),3,0))"  '4000.00' '0.005'
Near 'Depreciate: two-monthly depreciation unchanged' "SUM(INDEX($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 12, 0) * 2)),3,0))" '4000.00' '0.005'
Near 'Depreciate: six-monthly depreciation unchanged' "SUM(INDEX($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 6)),3,0))"  '8000.00' '0.005'
Near 'Depreciate: yearly depreciation unchanged'      "SUM(INDEX($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 6, 0) * 12)),3,0))" '10000.00' '0.005'
Near 'Depreciate: monthly block unchanged'     "SUM($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 24, 0))))"      '397999.12' '0.005'
Near 'Depreciate: quarterly block unchanged'   "SUM($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 3)))"   '141999.76' '0.005'
Near 'Depreciate: two-monthly block unchanged' "SUM($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 12, 0) * 2)))"  '205999.60' '0.005'
Near 'Depreciate: six-monthly block unchanged' "SUM($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 6)))"   '113999.84' '0.005'
Near 'Depreciate: yearly block unchanged'      "SUM($dp($dpA, EDATE(DATE(2026,1,1), SEQUENCE( , 6, 0) * 12)))"  '70000.00'  '0.005'
# Twenty-day periods and an uneven sub-monthly timeline both have to collect the whole year.
Near 'Depreciate: twenty-day periods collect a full year' `
     "SUM(INDEX($dp($dpA, DATE(2026,1,1) + SEQUENCE( , 18, 0) * 20),3,0)) - 2000" '0' '0.005'
Near 'Depreciate: an uneven timeline collects a full year' `
     "SUM(INDEX($dp($dpA, $uneven),3,0)) - 2000" '0' '0.005'
# A life in years is a life, not a date. Transposing arguments two and three puts 46,023
# where the life belongs and asks for 552,276 months of schedule.
$dpTL = "EDATE(DATE(2026,1,1), SEQUENCE( , 12, 0))"
Same 'Depreciate: a date where the life belongs is refused' `
     "LEFT(INDEX($dp(10000, 5, DATE(2026,1,1), $dpTL),1,1),11)" 'LifeInYears'
Same 'Depreciate: a life of 101 years is refused' `
     "LEFT(INDEX($dp(10000, DATE(2026,1,1), 101, $dpTL),1,1),11)" 'LifeInYears'
Same 'Depreciate: a life of nought is refused' `
     "LEFT(INDEX($dp(10000, DATE(2026,1,1), 0, $dpTL),1,1),11)" 'LifeInYears'
Same 'Depreciate: a life in words is refused' `
     "LEFT(INDEX($dp(10000, DATE(2026,1,1), `"five`", $dpTL),1,1),11)" 'LifeInYears'
Near 'Depreciate: a life of 100 years is still a life' `
     "SUM(INDEX($dp(10000, DATE(2026,1,1), 100, $dpTL),3,0)) - 100" '0' '0.5'
Same 'Depreciate: help with no args' "INDEX($dp(),1,1)" 'FUNCTION:'

# --- Allocate. The accumulator was rebuilt with HSTACK on every REDUCE pass, so the work
# was quadratic in the number of amounts; it is a closed form and is now computed one
# column at a time. Every published result has to come back to the digit, the deliberate
# last-column adjustment included.
$al = "oz.Allocate$L"
Same 'Allocate: documented example' `
     "TEXTJOIN(`",`",FALSE,$al({999.99,2000},`"Y`",`"Q`"))" '250,250,250,249.99,500,500,500,500'
Same 'Allocate: quarters to quarters is the identity' `
     "TEXTJOIN(`",`",FALSE,$al({999.99,2000},`"Q`",`"Q`"))" '999.99,2000'
Same 'Allocate: quarters to months' `
     "TEXTJOIN(`",`",FALSE,$al({999.99,2000},`"Q`",`"M`"))" '333.33,333.33,333.33,666.67,666.67,666.66'
Same 'Allocate: default is years to months' `
     "TEXTJOIN(`",`",FALSE,$al(1200))" '100,100,100,100,100,100,100,100,100,100,100,100'
Same 'Allocate: a single amount' "TEXTJOIN(`",`",FALSE,$al(1000,`"Y`",`"Q`"))" '250,250,250,250'
Same 'Allocate: negatives carry the adjustment too' `
     "TEXTJOIN(`",`",FALSE,$al({-100.01},`"Y`",`"Q`"))" '-25,-25,-25,-25.01'
Near 'Allocate: years to weeks sums back to the amount' "SUM($al({100},`"Y`",`"W`")) - 100" '0' '0.0000001'
Near 'Allocate: quarters to weeks sums back to the amount' "SUM($al({100},`"Q`",`"W`")) - 100" '0' '0.0000001'
Near 'Allocate: 400 amounts give 4,800 columns' "COLUMNS($al(SEQUENCE(,400,100,0),`"Y`",`"M`")) - 4800" '0'
Near 'Allocate: 400 amounts still sum' "SUM($al(SEQUENCE(,400,100,0),`"Y`",`"M`")) - 40000" '0' '0.0000001'
Same 'Allocate: help with no args' "INDEX($al(),1,1)" 'FUNCTION:'

# --- InterestLRV. It solved for a principal repayment larger than the principal wherever
# the cash available exceeded the debt, which put the average balance below half the
# opening balance and understated the interest. Its caller capped the payment; it did not.
Near 'InterestLRV: documented example'  "$ilrv(6666.37, 3.5, 90000, 0.03/12)" '222.90' '0.005'
Near 'InterestLRV: cash over the debt charges half a period' "$ilrv(1200, 1, 1000, 0.05)" '25' '0.0000001'
Near 'InterestLRV: cash far over the debt charges half a period' "$ilrv(5000, 1, 1000, 0.01)" '5' '0.0000001'
Near 'InterestLRV: below the cap is unchanged' "$ilrv(300, 1.2, 1000, 0.005)" '4.3859649' '0.0000001'
# A cap that binds must not change what the caller reports as repaid, only the interest.
Near 'InterestLRV: the sculpted schedule still retires the principal exactly' `
     "SUM(INDEX($lrv(, {1000,0,0,0,0}, {300,300,300,300,300}, {1.2,1.2,1.2,1.2,1.2}, {0.06,0.06,0.06,0.06,0.06}, 12), 3, 0))" '-1000' '0.0000001'
# and the period the debt is retired in now carries interest rather than nothing
Near 'InterestLRV: the retiring period is no longer free' `
     "INDEX($lrv(, {1000,0,0}, {2000,2000,2000}, {1,1,1}, {0.06,0.06,0.06}, 12), 2, 1) - 30" '0' '0.005'

# --- The last two help-text misspellings in the library.
$tp = "oz.TimelinePosition$L"
$ld = "oz.LabelDepreciate$L"
Same 'TimelinePosition: help with no args' "INDEX($tp(),1,1)" 'FUNCTION:'
Near 'TimelinePosition: its table spells timeline' `
     "SUMPRODUCT(--ISNUMBER(SEARCH(`"timline`",$tp())))" '0'
Near 'LabelDepreciate: its table spells the' `
     "SUMPRODUCT(--ISNUMBER(SEARCH(`" teh `",$ld())))" '0'

# --- AASB 16 leases. The liability is checked against NPV(), which is Excel's own
# discounting and so an independent oracle rather than a restatement of the same
# arithmetic. The schedule is checked by identity: a corkscrew that closes to nil and
# rolls forward is right whatever the rate, and both identities name a row and vary
# down the row, because a whole-block total cancels the very row under test.
$lli  = "oz.LeaseLiability$L"
$lsch = "oz.LeaseSchedule$L"
$lrou = "oz.ROUSchedule$L"
$lrem = "oz.LeaseRemeasure$L"

Near 'Lease liability matches NPV in arrears' "$lli({100,100,100},0.05)" 'NPV(0.05,100,100,100)'
Near 'Lease liability uneven payments match NPV' `
     "$lli({120,100,80,60},0.0075)" 'NPV(0.0075,120,100,80,60)'
# The first payment in advance is paid at commencement and is not part of the liability.
Near 'Lease liability in advance' "$lli({100,100,100},0.05,TRUE)" 'NPV(0.05,100,100)'
Near 'Lease liability with only a commencement payment is nil' `
     "$lli({100},0.05,TRUE)" '0'
Near 'Lease liability reads a column the same as a row' `
     "$lli({100;100;100},0.05)-$lli({100,100,100},0.05)" '0'
Near 'Lease liability at a nil rate is the undiscounted total' "$lli({100,100,100},0)" '300'
Same 'Lease liability: help with no args' "INDEX($lli(),1,1)" 'FUNCTION:'

# The schedule must close to nil on any rate, any term and either timing, because its
# opening balance is by construction the present value of the payments it then unwinds.
foreach ($rate in '0', '0.0025', '0.05', '0.15') {
    foreach ($n in '1', '2', '5', '12') {
        foreach ($adv in 'FALSE', 'TRUE') {
            $p = "SEQUENCE(,$n,100,0)"
            $s = "$lsch($p,$rate,$adv)"
            $columns = if ($adv -eq 'TRUE') { [Math]::Max(1, [int]$n - 1) } else { [int]$n }
            Near "Lease schedule closes to nil, rate $rate, $n periods, advance $adv" `
                 "INDEX($s,4,$columns)" '0'
            Near "Lease schedule shape, rate $rate, $n periods, advance $adv" `
                 "ROWS($s)*1000+COLUMNS($s)" "4000+$columns"
            # Closing = opening less payment plus interest, in every period, either timing.
            Near "Lease schedule reconciles each period, rate $rate, $n periods, advance $adv" `
                 ("SUMPRODUCT(ABS(CHOOSEROWS($s,4)-CHOOSEROWS($s,1)" +
                  "+CHOOSEROWS($s,2)-CHOOSEROWS($s,3)))") '0'
            Near "Lease schedule never goes negative, rate $rate, $n periods, advance $adv" `
                 "SUMPRODUCT(--(CHOOSEROWS($s,4)<-0.000001))" '0'
        }
    }
}
# Each opening balance is the closing balance before it. One period has none to compare.
foreach ($adv in 'FALSE', 'TRUE') {
    $s = "$lsch(SEQUENCE(,6,100,0),0.04,$adv)"
    $payments = if ($adv -eq 'TRUE') { 'DROP(SEQUENCE(,6,100,0),,1)' } else { 'SEQUENCE(,6,100,0)' }
    Near "Lease schedule rolls forward, advance $adv" `
         "SUMPRODUCT(ABS(DROP(CHOOSEROWS($s,1),,1)-DROP(CHOOSEROWS($s,4),,-1)))" '0'
    Near "Lease schedule opens on the liability, advance $adv" `
         "INDEX($s,1,1)-$lli(SEQUENCE(,6,100,0),0.04,$adv)" '0'
    Near "Lease schedule echoes its payments, advance $adv" `
         "SUMPRODUCT(ABS(CHOOSEROWS($s,2)-$payments))" '0'
}
# Total interest is the cash paid less what was recognised as a liability.
Near 'Lease schedule interest totals payments less liability' `
     "SUM(CHOOSEROWS($lsch({100,100,100},0.05),3))" 'SUM({100,100,100})-NPV(0.05,100,100,100)'
# A single payment in advance is settled before any interest can accrue.
Near 'Lease schedule, one payment in advance, no interest' "INDEX($lsch({100},0.05,TRUE),3,1)" '0'
Near 'Lease schedule, one payment in arrears, interest on the discounted balance' `
     "INDEX($lsch({100},0.05),3,1)" '100-100/1.05'
Same 'Lease schedule: help with no args' "INDEX($lsch(),1,1)" 'FUNCTION:'

# --- Right-of-use asset
foreach ($n in '1', '3', '12', '60') {
    Near "ROU depreciation sums to cost, $n periods" "SUM(CHOOSEROWS($lrou(1200,$n),2))" '1200'
    Near "ROU closes to nil, $n periods" "INDEX($lrou(1200,$n),3,$n)" '0'
    Near "ROU opens at cost, $n periods" "INDEX($lrou(1200,$n),1,1)" '1200'
    Near "ROU shape, $n periods" "ROWS($lrou(1200,$n))*1000+COLUMNS($lrou(1200,$n))" "3000+$n"
}
Same 'ROU documented example' `
     "TEXTJOIN(`",`",FALSE,TEXT($lrou(300,3),`"0`"))" '300,200,100,100,100,100,200,100,0'
Same 'ROU refuses nought periods' "$lrou(300,0)" "ROUSchedule$L needs a numeric Cost and Periods of at least 1"
Same 'ROU refuses text periods' "$lrou(300,`"three`")" "ROUSchedule$L needs a numeric Cost and Periods of at least 1"
Same 'ROU: help with no args' "INDEX($lrou(),1,1)" 'FUNCTION:'

# The pair is what makes AASB 16 front-loaded: interest falls while depreciation does not,
# so the first period costs more than the last even though the rent never changes.
Near 'Lease expense is front-loaded' `
     ("--(INDEX($lsch(SEQUENCE(,12,100,0),0.005),3,1)+INDEX($lrou($lli(SEQUENCE(,12,100,0),0.005),12),2,1)" +
      ">INDEX($lsch(SEQUENCE(,12,100,0),0.005),3,12)+INDEX($lrou($lli(SEQUENCE(,12,100,0),0.005),12),2,12))") '1'

# --- Remeasurement
Same 'Remeasure documented example' `
     "TEXTJOIN(`",`",FALSE,TEXT($lrem({110,110},0.05,185.94,181.55),`"0.00`"))" '204.54,18.60,200.15,0.00'
Near 'Remeasure restates the liability' `
     "INDEX($lrem({110,110},0.05,185.94,181.55),1,1)-$lli({110,110},0.05)" '0'
Near 'Remeasure adjustment is the movement' `
     "INDEX($lrem({110,110},0.05,185.94,181.55),2,1)-($lli({110,110},0.05)-185.94)" '0'
Near 'Remeasure takes the adjustment to the asset' `
     "INDEX($lrem({110,110},0.05,185.94,181.55),3,1)-(181.55+$lli({110,110},0.05)-185.94)" '0'
Near 'Remeasure leaves profit or loss alone while the asset absorbs it' `
     "INDEX($lrem({110,110},0.05,185.94,181.55),4,1)" '0'
# Paragraph 39: the asset stops at nil and the rest is recognised in profit or loss.
Near 'Remeasure floors the asset at nil' "INDEX($lrem({10,10},0.05,185.94,5),3,1)" '0'
Near 'Remeasure takes the remainder to profit or loss' `
     "INDEX($lrem({10,10},0.05,185.94,5),4,1)-(5+$lli({10,10},0.05)-185.94)" '0'
# Whatever the split, the asset and the remainder together are the unfloored movement.
foreach ($rou0 in '0', '5', '50', '400') {
    Near "Remeasure splits without loss, asset $rou0" `
         ("INDEX($lrem({10,10},0.05,185.94,$rou0),3,1)+INDEX($lrem({10,10},0.05,185.94,$rou0),4,1)" +
          "-($rou0+INDEX($lrem({10,10},0.05,185.94,$rou0),2,1))") '0'
}
Near 'Remeasure honours payments in advance' `
     "INDEX($lrem({110,110},0.05,185.94,181.55,TRUE),1,1)-NPV(0.05,110)" '0'
Same 'Remeasure: help with no args' "INDEX($lrem(),1,1)" 'FUNCTION:'

# --- Return on equity. Net income over AVERAGE shareholders' equity, which is what the
# help and the page it links have always said; until 6 Sep 2026 the function divided by
# whatever it was handed. The two-argument call must return exactly what it did, because
# the ratios worksheet caches it.
$roe = "oz.ROE$L"
Near 'ROE: two arguments, as cached'   "$roe(59.972, AVERAGE(251.635, 256.144))" '0.236213' '0.0000005'
Near 'ROE: opening equity averages in' "$roe(59.972, 256.144, 251.635)"          '0.236213' '0.0000005'
Near 'ROE: blank opening cell = closing' "$roe(59.972, 253.8895, Z1)"            '0.236213' '0.0000005'
Near 'ROE: rows of periods'            "SUM($roe({10,20}, {100,200}, {100,200}))" '0.2' '0.0000001'
Near 'Sheet: ratios ROE unchanged'     "'oz.FinancialRatios'!A47"                 '0.236213' '0.0000005'

# --- Effective rate conversion, the rate per period the lease functions take.
$pr = "oz.PeriodRate$L"
$ar = "oz.AnnualRate$L"
Near 'PeriodRate: 5% a year is 0.4074% a month' "$pr(0.05)"       '0.00407412' '0.000000005'
Near 'PeriodRate: one period a year is itself'  "$pr(0.05, 1)"    '0.05'       '0.0000000001'
Near 'PeriodRate: quarterly'                    "$pr(0.05, 4)"    '0.01227223' '0.000000005'
Near 'PeriodRate: blank periods cell = 12'      "$pr(0.05, Z1)"   '0.00407412' '0.000000005'
Near 'PeriodRate: a row of rates'               "SUM($pr({0.05,0.05}))" '0.00814825' '0.00000001'
Near 'AnnualRate: 1% a month is 12.68% a year'  "$ar(0.01)"       '0.12682503' '0.000000005'
Near 'AnnualRate: quarterly'                    "$ar(0.01, 4)"    '0.04060401' '0.000000005'
Near 'AnnualRate: agrees with EFFECT()'         "$ar(0.06/12) - EFFECT(0.06, 12)" '0' '0.0000000001'

# --- Day count conventions. July 2026 has 31 days, September 30, and the year 365.
$dc = "oz.DayCountRate$L"
$jul = "EDATE(DATE(2026,7,1), {0,1,2})"
Same 'DayCount: Actual/365 over Jul to Sep'   "TEXTJOIN(`",`",FALSE,TEXT($dc($jul, 0.073),`"0.0000`"))"    '0.0062,0.0062,0.0060'
Same 'DayCount: default convention is 3'      "TEXTJOIN(`",`",FALSE,TEXT($dc($jul, 0.073, Z1),`"0.0000`"))" '0.0062,0.0062,0.0060'
Near 'DayCount: 30/360 is a flat twelfth'     "SUMPRODUCT(ABS($dc($jul, 0.073, 1) - 0.073/12))" '0' '0.0000000001'
Near 'DayCount: Actual/360, first period'     "INDEX($dc($jul, 0.073, 2),1,1) - 0.073*31/360"  '0' '0.0000000001'
Near 'DayCount: end dates give the same months' "SUMPRODUCT(ABS($dc(EOMONTH(DATE(2026,7,1), {0,1,2}), 0.073, 3, TRUE) - $dc($jul, 0.073)))" '0' '0.0000000001'
Near 'DayCount: a column timeline'            "SUMPRODUCT(ABS($dc(TRANSPOSE($jul), 0.073) - $dc($jul, 0.073)))" '0' '0.0000000001'
Near 'DayCount: text dates'                   "INDEX($dc({`"2026-07-01`",`"2026-08-01`"}, 0.073),1,1) - 0.073*31/365" '0' '0.0000000001'
Near 'DayCount: one APR per period'           "INDEX($dc($jul, {0.05,0.06,0.07}),1,3) - 0.07*30/365" '0' '0.0000000001'
Near 'DayCount: Actual/Actual, leap February' "INDEX($dc(EDATE(DATE(2028,1,1), {0,1,2}), 0.073, 4),1,2) - 0.073*29/366" '0' '0.0000000001'
Near 'DayCount: weekly timeline'              "INDEX($dc(DATE(2026,7,1) + {0,7,14}, 0.073),1,3) - 0.073*7/365" '0' '0.0000000001'
# Actual/Actual on a week that straddles 31 December: four days in one year, three in the
# next, each over its own year's length. Common to leap, then leap to common.
Near 'DayCount: Actual/Actual, common into leap' "INDEX($dc(DATE(2023,12,28) + {0,7,14}, 0.073, 4),1,1) - 0.073*(4/365 + 3/366)" '0' '0.0000000001'
Near 'DayCount: Actual/Actual, leap into common' "INDEX($dc(DATE(2024,12,28) + {0,7,14}, 0.073, 4),1,1) - 0.073*(4/366 + 3/365)" '0' '0.0000000001'
Near 'DayCount: Actual/Actual, financial year'  "INDEX($dc(EDATE(DATE(2023,7,1), {0,12,24}), 0.073, 4),1,1) - 0.073*(184/365 + 182/366)" '0' '0.0000000001'
Near 'DayCount: one date is an error'         "--ISERROR($dc(DATE(2026,7,1), 0.073))" '1'
Near 'DayCount: unknown convention is an error' "--ISERROR($dc($jul, 0.073, 5))" '1'
Near 'DayCount: EndDates given as 1'          "SUMPRODUCT(ABS($dc(EOMONTH(DATE(2026,7,1), {0,1,2}), 0.073, 3, 1) - $dc($jul, 0.073)))" '0' '0.0000000001'
Near 'DayCount: a column of APRs gives a row' "COLUMNS($dc(TRANSPOSE($jul), {0.05;0.06;0.07}))*10 + ROWS($dc(TRANSPOSE($jul), {0.05;0.06;0.07}))" '31'
Near 'DayCount: month-end starts, last period' "INDEX($dc(EDATE(DATE(2025,12,31), {0,1,2}), 0.073),1,3) - 0.073*31/365" '0' '0.0000000001'
Near 'DayCount: month-end ends, February first' "INDEX($dc(EOMONTH(DATE(2025,2,1), {0,1,2}), 0.073, 3, TRUE),1,1) - 0.073*28/365" '0' '0.0000000001'
Same 'DayCount: help with no args'            "INDEX($dc(),1,1)" 'FUNCTION:'

# --- DateDif. A month after 31 January is the last day of February, and every remainder
# is counted from that anniversary. Excel's own DATEDIF gets the MD case wrong.
$dd = "oz.DateDif$L"
Near 'DateDif: MD, 31 Jan to 1 Mar'    "$dd(DATE(2026,1,31), DATE(2026,3,1), `"MD`")" '1'
Near 'DateDif: M, 31 Jan to 1 Mar'     "$dd(DATE(2026,1,31), DATE(2026,3,1), `"M`")"  '1'
Near 'DateDif: M, 31 Jan to 29 Feb'    "$dd(DATE(2024,1,31), DATE(2024,2,29), `"M`")" '1'
Near 'DateDif: YD across a leap day'   "$dd(DATE(2024,3,15), DATE(2025,3,10), `"YD`")" '360'
Near 'DateDif: Y'                      "$dd(DATE(2024,3,15), DATE(2025,3,10), `"Y`")"  '0'
Near 'DateDif: YM'                     "$dd(DATE(2020,5,10), DATE(2026,3,10), `"YM`")" '10'
Near 'DateDif: default unit is days'   "$dd(DATE(2026,1,1), DATE(2026,1,31))"        '30'
Near 'DateDif: lower-case unit'        "$dd(DATE(2020,5,10), DATE(2026,3,10), `"y`")"  '5'
Near 'DateDif: text dates'             "$dd(`"2026-01-01`", `"2026-12-31`", `"M`")"    '11'
Near 'DateDif: same day is nought'     "$dd(DATE(2026,1,1), DATE(2026,1,1), `"M`")"   '0'
Near 'DateDif: reversed range errors'  "--ISERROR($dd(DATE(2026,3,1), DATE(2026,1,1)))" '1'
Near 'DateDif: unknown unit errors'    "--ISERROR($dd(DATE(2026,1,1), DATE(2026,3,1), `"X`"))" '1'
Same 'DateDif: help with no args'      "INDEX($dd(),1,1)" 'FUNCTION:'

# --- Debt sculpting on period rates. A row of rates from a day count convention replaces
# the flat twelfth of APR, and APR may then be left out altogether.
$rates = "$dc(EDATE(DATE(2026,7,1), {0,1,2}), 0.073)"
$onrates = "$dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, , , $rates)"
Near 'Debt: PeriodRates charge the day count rate' "INDEX($onrates,2,1) - 1000*0.073*31/365" '0' '0.0000001'
Near 'Debt: PeriodRates ignore APR and months'    "SUMPRODUCT(ABS($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.9,0.9,0.9}, 12, $rates) - $onrates))" '0' '0.0000001'
Near 'Debt: PeriodRates keep the roll-forward'    "SUMPRODUCT(ABS(INDEX($onrates,4,0) - INDEX($onrates,1,0) - INDEX($onrates,2,0) - INDEX($onrates,3,0)))" '0' '0.0000001'
Near 'Debt: without either rate row it is help'  "--ISTEXT(INDEX($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}),1,1))" '1'
# Interest on the average balance solves I = r * (P - (C - I) / 2) with C = 300 / 1.2 the cash
# for debt service, so I = r * (P - C / 2) / (1 - r / 2) while the cash clears less than the
# principal. InterestLRV iterates to within a cent of that.
Near 'Debt: LRV on PeriodRates, first interest'   "LET(r, 0.073*31/365, INDEX($lrv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, , , $rates),2,1) - r*(1000-125)/(1-r/2))" '0' '0.001'
Near 'Debt: blank PeriodRates cell is not given'  "SUMPRODUCT(ABS($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12, Z1) - $dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt: blank PeriodRates range is not given' "SUMPRODUCT(ABS($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12, Z1:Z3) - $dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt: LRV on PeriodRates ends at zero'      "INDEX($lrv(, {1000,0,0}, {1800,1800,1800}, {1.2,1.2,1.2}, , , $rates),4,1)" '0' '0.0000001'

# --- Randomised checks. Each trial draws fresh inputs, compares the function with an
# independent Excel formula and, on a miss, reports the inputs that produced it so the
# case can be reproduced by hand. A run with no failing trial reads OK. Excel has no
# seeded generator, so a failure here names its own inputs rather than a seed.
function Fuzz($id, $draw, $show, $expr, $want, $tol = '0.005', $trials = 25) {
    $trial = "LET($draw, got, $expr, want, $want, IF(ABS(got-(want))<$tol, `"`", `"got `"&TEXT(got,`"0.000000`")&`" want `"&TEXT(want,`"0.000000`")&`" for `"&$show))"
    Check $id "=LET(r, MAP(SEQUENCE($trials), LAMBDA(i, $trial)), bad, FILTER(r, r<>`"`", `"`"), IF(INDEX(bad,1,1)=`"`", `"OK`", INDEX(bad,1,1)))"
}
$amb = "oz.AmortiseB$L"
$am  = "oz.Amortise$L"
$ll  = "oz.LeaseLiability$L"

# A level loan's balance after k payments is what FV() says it is.
Fuzz 'fuzz: AmortiseB closing balance = -FV()' `
     'p, RANDBETWEEN(1000,500000), a, RANDBETWEEN(1,15)/100, n, RANDBETWEEN(6,48), k, RANDBETWEEN(1,n)' `
     '"p="&p&" a="&a&" n="&n&" k="&k' `
     "INDEX($amb(p, a, n, n), 4, k)" '-FV(a/12, k, PMT(a/12, n, p), p)'
# Total interest over the term is total payments less principal.
Fuzz 'fuzz: Amortise total interest = PMT()*n - p' `
     'p, RANDBETWEEN(1000,500000), a, RANDBETWEEN(1,15)/100, n, RANDBETWEEN(6,120)' `
     '"p="&p&" a="&a&" n="&n' `
     "SUM(INDEX($am(p, a, n, DATE(2026,7,1)), 3, 0))" '-PMT(a/12, n, p)*n - p'
# Level lease payments are an annuity: PV() in arrears, and PV() over one fewer in advance.
Fuzz 'fuzz: LeaseLiability in arrears = -PV()' `
     'm, RANDBETWEEN(100,5000), r, RANDBETWEEN(1,12)/1200, n, RANDBETWEEN(1,60)' `
     '"m="&m&" r="&r&" n="&n' `
     "$ll(EXPAND(m, 1, n, m), r)" '-PV(r, n, m)'
Fuzz 'fuzz: LeaseLiability in advance = -PV() over n-1' `
     'm, RANDBETWEEN(100,5000), r, RANDBETWEEN(1,12)/1200, n, RANDBETWEEN(2,60)' `
     '"m="&m&" r="&r&" n="&n' `
     "$ll(EXPAND(m, 1, n, m), r, TRUE)" '-PV(r, n-1, m)'
# Straight line is SLN(); diminishing value opens at twice the straight-line rate, capped.
Fuzz 'fuzz: PrimeCost first period = SLN()' `
     'c, RANDBETWEEN(1000,900000), n, RANDBETWEEN(1,40)' '"c="&c&" n="&n' `
     "INDEX($pc(c, n),1,1)" 'SLN(c, 0, n)'
Fuzz 'fuzz: DiminishingValue first period = 200% rate' `
     'c, RANDBETWEEN(1000,900000), n, RANDBETWEEN(1,40)' '"c="&c&" n="&n' `
     "INDEX($dv(c, n),1,1)" 'c*MIN(1, 2/n)'
Fuzz 'fuzz: DiminishingValue sums to cost' `
     'c, RANDBETWEEN(1000,900000), n, RANDBETWEEN(1,40)' '"c="&c&" n="&n' `
     "SUM($dv(c, n))" 'c'
# Return on equity is net income over the average of opening and closing equity.
Fuzz 'fuzz: ROE = income / AVERAGE(open, close)' `
     'ni, RANDBETWEEN(-500,5000), ce, RANDBETWEEN(1000,90000), oe, RANDBETWEEN(1000,90000)' `
     '"ni="&ni&" ce="&ce&" oe="&oe' `
     "$roe(ni, ce, oe)" 'ni/AVERAGE(oe, ce)' '0.0000001'
# The two rate converters invert each other, and the monthly one agrees with NOMINAL().
Fuzz 'fuzz: AnnualRate(PeriodRate(a)) = a' `
     'a, RANDBETWEEN(1,2000)/10000, n, CHOOSE(RANDBETWEEN(1,4),1,4,12,52)' '"a="&a&" n="&n' `
     "$ar($pr(a, n), n)" 'a' '0.000000001'
Fuzz 'fuzz: PeriodRate = NOMINAL()/12' `
     'a, RANDBETWEEN(1,2000)/10000' '"a="&a' `
     "$pr(a)" 'NOMINAL(a, 12)/12' '0.000000001'
# A whole calendar year of monthly rates sums to the APR under the convention that
# matches the year: Actual/365 in a common year, Actual/Actual in a leap year, and
# 30/360 gives a flat twelfth in any year. End-date timelines give the same rates.
Fuzz 'fuzz: DayCount Actual/365 sums to APR in a common year' `
     'a, RANDBETWEEN(1,1500)/10000, y, CHOOSE(RANDBETWEEN(1,3), 2025, 2026, 2027)' '"a="&a&" y="&y' `
     "SUM($dc(EDATE(DATE(y,1,1), SEQUENCE(1,12,0)), a))" 'a' '0.000000001'
Fuzz 'fuzz: DayCount Actual/Actual sums to APR in a leap year' `
     'a, RANDBETWEEN(1,1500)/10000, y, CHOOSE(RANDBETWEEN(1,2), 2024, 2028)' '"a="&a&" y="&y' `
     "SUM($dc(EDATE(DATE(y,1,1), SEQUENCE(1,12,0)), a, 4))" 'a' '0.000000001'
Fuzz 'fuzz: DayCount 30/360 is APR/12 every month' `
     'a, RANDBETWEEN(1,1500)/10000, y, RANDBETWEEN(2020,2030), s, RANDBETWEEN(1,12)' '"a="&a&" y="&y&" s="&s' `
     "SUMPRODUCT(ABS($dc(EDATE(DATE(y,s,1), SEQUENCE(1,12,0)), a, 1) - a/12))" '0' '0.000000001'
Fuzz 'fuzz: DayCount end dates match start dates' `
     'a, RANDBETWEEN(1,1500)/10000, y, RANDBETWEEN(2020,2030), s, RANDBETWEEN(1,12)' '"a="&a&" y="&y&" s="&s' `
     "SUMPRODUCT(ABS($dc(EOMONTH(DATE(y,s,1), SEQUENCE(1,12,0)), a, 3, TRUE) - $dc(EDATE(DATE(y,s,1), SEQUENCE(1,12,0)), a)))" '0' '0.000000001'
Fuzz 'fuzz: DayCount Actual/360 = days/360' `
     'a, RANDBETWEEN(1,1500)/10000, y, RANDBETWEEN(2020,2030), s, RANDBETWEEN(1,12)' '"a="&a&" y="&y&" s="&s' `
     "INDEX($dc(EDATE(DATE(y,s,1), {0,1}), a, 2),1,1)" 'a*(EDATE(DATE(y,s,1),1)-DATE(y,s,1))/360' '0.000000001'
# Where DATEDIF is right, on start days up to the 28th, the two agree.
Fuzz 'fuzz: DateDif M = DATEDIF M' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), RANDBETWEEN(1,28)), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e, `"M`")" 'DATEDIF(s, e, "M")' '0.5'
Fuzz 'fuzz: DateDif Y = DATEDIF Y' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), RANDBETWEEN(1,28)), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e, `"Y`")" 'DATEDIF(s, e, "Y")' '0.5'
Fuzz 'fuzz: DateDif YM = DATEDIF YM' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), RANDBETWEEN(1,28)), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e, `"YM`")" 'DATEDIF(s, e, "YM")' '0.5'
Fuzz 'fuzz: DateDif D = days' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), RANDBETWEEN(1,28)), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e)" 'e - s' '0.5'
# On month-end start days, where DATEDIF is not a reference, a complete month is an
# EDATE() anniversary that has arrived: count them independently and take the days
# past the last one.
Fuzz 'fuzz: DateDif M on days 29 to 31 = anniversaries reached' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), 28) + RANDBETWEEN(1,3), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e, `"M`")" 'SUMPRODUCT(--(EDATE(s, SEQUENCE(1,200)) <= e))' '0.5'
Fuzz 'fuzz: DateDif MD on days 29 to 31 = days past the last anniversary' `
     's, DATE(RANDBETWEEN(2000,2030), RANDBETWEEN(1,12), 28) + RANDBETWEEN(1,3), e, s + RANDBETWEEN(0,4000)' `
     '"s="&TEXT(s,"yyyy-mm-dd")&" e="&TEXT(e,"yyyy-mm-dd")' `
     "$dd(s, e, `"MD`")" 'e - EDATE(s, SUMPRODUCT(--(EDATE(s, SEQUENCE(1,200)) <= e)))' '0.5'
# Debt sculpting on period rates charges opening balance times the period's rate, and
# the LRV variant charges interest on the average balance: I = r * (P - C / 2) / (1 - r / 2)
# with C = 300 / 1.2 the cash for debt service, solved to within a cent by InterestLRV.
Fuzz 'fuzz: DebtSculptVariable interest = opening * period rate' `
     'd, RANDBETWEEN(1000,100000), r, RANDBETWEEN(1,20)/1000' '"d="&d&" r="&r' `
     "INDEX($dsv(, HSTACK(d,0,0), {300,300,300}, {1.2,1.2,1.2}, , , HSTACK(r,r,r)), 2, 1)" 'd*r' '0.0000001'
Fuzz 'fuzz: DebtSculptVariableLRV interest on the average balance' `
     'd, RANDBETWEEN(1000,100000), r, RANDBETWEEN(1,20)/1000' '"d="&d&" r="&r' `
     "INDEX($lrv(, HSTACK(d,0,0), {300,300,300}, {1.2,1.2,1.2}, , , HSTACK(r,r,r)), 2, 1)" 'r*(d-125)/(1-r/2)' '0.001'
# The randomised checks rest on MAP() drawing fresh values for every trial. Prove it.
Near 'fuzz: MAP draws fresh values per trial' "--(ROWS(UNIQUE(MAP(SEQUENCE(25), LAMBDA(i, RANDBETWEEN(1,1000000))))) > 1)" '1'

$xl = $null; $wb = $null; $tmp = $null; $exit = 0

# Excel rejects incoming COM calls while it is mid-calculation (RPC_E_CALL_REJECTED),
# so anything that can arrive at a busy moment gets retried with a backoff.
function Invoke-Excel([scriptblock]$op, [int]$tries = 12) {
    for ($i = 1; $i -le $tries; $i++) {
        try { return & $op }
        catch {
            if ($i -eq $tries) { throw }
            Start-Sleep -Milliseconds (250 * $i)
        }
    }
}

# Excel's COM server is shared: New-Object attaches to an already running EXCEL.EXE, and
# this script calls Quit() when it finishes. Refuse to run rather than close someone's
# open workbooks underneath them.
# A previous run's instance can take a moment to exit, so give it a few seconds first.
for ($w = 0; $w -lt 10 -and @(Get-Process EXCEL -ErrorAction SilentlyContinue).Count -gt 0; $w++) {
    Start-Sleep -Milliseconds 500
}
if (@(Get-Process EXCEL -ErrorAction SilentlyContinue).Count -gt 0) {
    Write-Output 'Excel is already running. Close it first: this test drives Excel over COM and quits it when done.'
    exit 2
}

try {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false; $xl.DisplayAlerts = $false
    $xl.AutomationSecurity = 1; $xl.AskToUpdateLinks = $false; $xl.EnableEvents = $false

    # Excel's COM server intermittently refuses the first call after a prior instance quits.
    $wb = Invoke-Excel { $xl.Workbooks.Open($Path, 0, $false) }
    if ($null -eq $wb) { throw "could not open $Path" }

    $sw = [Diagnostics.Stopwatch]::StartNew()
    Invoke-Excel { $xl.CalculateFullRebuild() }
    $sw.Stop()

    Write-Output ("Excel {0} build {1}" -f $xl.Version, $xl.Build)
    Write-Output ("{0}: {1} sheets, {2} defined names, full rebuild {3:N2}s" -f
        (Split-Path $Path -Leaf), $wb.Worksheets.Count, $wb.Names.Count, $sw.Elapsed.TotalSeconds)
    Write-Output ''

    # --- every cell in the workbook, after a real recalculation
    $errors = @(); $formulaCount = 0
    foreach ($ws in $wb.Worksheets) {
        $used = $ws.UsedRange
        try { $formulaCount += $used.SpecialCells(-4123).Count } catch {}
        foreach ($kind in -4123, 2) {
            try { $bad = $used.SpecialCells($kind, 16) } catch { continue }
            foreach ($cell in $bad) {
                $errors += ("{0}!{1} = {2}   <= {3}" -f $ws.Name, $cell.Address($false, $false),
                            $cell.Text, $cell.Formula2)
            }
        }
    }
    Write-Output ("Recalculated cells: {0} formulas, {1} in error" -f $formulaCount, $errors.Count)
    if ($errors.Count) { $errors | ForEach-Object { Write-Output ("  " + $_) }; $exit = 1 }

    # --- assertions on a scratch sheet.
    # Written and read as whole ranges: a call per cell gets rejected while Excel is
    # busy recalculating, and 130 round-trips is slow besides.
    $tmp = Invoke-Excel { $wb.Worksheets.Add() }
    Invoke-Excel { $tmp.Name = 'zz_selftest' }
    Invoke-Excel { $tmp.Range('Z1:Z3').ClearContents() }   # the deliberate blank cells

    $grid = New-Object 'object[,]' $checks.Count, 1
    for ($i = 0; $i -lt $checks.Count; $i++) { $grid[$i, 0] = $checks[$i].f }
    $addr = 'A1:A' + $checks.Count
    # Hold calculation off until every probe is in place, so Excel is not recalculating
    # the sheet underneath the write.
    Invoke-Excel { $xl.Calculation = -4135 }        # xlCalculationManual
    Invoke-Excel { $tmp.Range($addr).Formula2 = $grid }
    Invoke-Excel { $xl.Calculation = -4105 }        # xlCalculationAutomatic
    Invoke-Excel { $xl.CalculateFullRebuild() }
    # Wait for the rebuild to finish. Reading a range Excel is still calculating hands
    # back a null, not a partial answer, and the assertion loop then indexes into it.
    # CalculationState comes back as the enum's NAME, so compare against the string, and
    # read it through Invoke-Excel: it is the one call certain to arrive while Excel is busy.
    $w = 0
    while ($w -lt 600 -and (Invoke-Excel { "$($xl.CalculationState)" }) -notin @('xlDone', '0')) {
        Start-Sleep -Milliseconds 500
        $w++
    }
    if ($w -ge 600) { throw 'Excel was still calculating after 5 minutes' }
    # leading comma stops PowerShell flattening the 2-D range value on the way out
    $vals = Invoke-Excel { , $tmp.Range($addr).Value2 }
    if ($null -eq $vals) { throw 'Excel returned no values for the assertion range' }

    $failed = @()
    for ($i = 0; $i -lt $checks.Count; $i++) {
        # Value2 hands back a 1-based 2-D array; GetValue avoids PowerShell's index parsing
        $got = if ($vals.Rank -eq 2) { [string]$vals.GetValue($i + 1, 1) } else { [string]$vals[$i] }
        if ($got -ne 'OK') { $failed += ("{0}: {1}" -f $checks[$i].id, $got) }
    }
    Write-Output ("Assertions: {0} run, {1} failed" -f $checks.Count, $failed.Count)
    if ($failed.Count) { $failed | ForEach-Object { Write-Output ("  FAIL " + $_) }; $exit = 1 }

    Invoke-Excel { $tmp.Delete() }; $tmp = $null
    Invoke-Excel { $wb.Close($false) }; $wb = $null

    Write-Output ''
    Write-Output $(if ($exit -eq 0) { 'PASS' } else { 'FAIL' })
}
catch {
    Write-Output ('FATAL: ' + $_.Exception.Message)
    Write-Output $_.ScriptStackTrace
    $exit = 1
}
finally {
    if ($tmp) { try { $tmp.Delete() } catch {} }
    if ($wb)  { try { $wb.Close($false) } catch {} }
    if ($xl)  { try { $xl.Quit() } catch {}; [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl) }
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
}
exit $exit
