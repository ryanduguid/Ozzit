# Copyright 2026 Ryan Duguid
# SPDX-License-Identifier: Apache-2.0

# End-to-end check of ozzit.xlsx in a real Excel calculation engine.
#
#   powershell -ExecutionPolicy Bypass -File tools\excel_selftest.ps1 [path\to\ozzit.xlsx]
#
# Requires Excel with LAMBDA support (Microsoft 365, or Excel 2024 and later). GitHub's
# runners have no Excel, so CI runs ozzit verify_workbook instead and this stays a
# local gate: verify_workbook checks the file's structure, this checks its arithmetic.
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
# own, generated from src/ by ozzit generate_selftest_examples. A help change means
# rerunning the generator; the ozzit tests fail when the fragment is stale.
. (Join-Path $PSScriptRoot 'selftest_examples.ps1')

$dv = "oz.DiminishingValue$L"
$pc = "oz.PrimeCost$L"
$ga = "oz.GSTAdd$L"
$ge = "oz.GSTExtract$L"
$mv = "oz.Movement$L"
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
# A negative life wrote the whole cost off in one period and a zero life divided by zero.
Same 'DV refuses a negative life' "IF(ISTEXT(INDEX($dv(1000,-5),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
Same 'PC refuses a zero life'     "IF(ISTEXT(INDEX($pc(1000,0),1,1)),`"diagnostic`",`"number`")" 'diagnostic'

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
# A column of rates is read row by row: one blank rate defaults its own row, and a
# zero-rated row stays zero. OR() once gave every row the default.
Near 'GST extract, per-row zero rate'  "INDEX($ge({110;110;110},{0.1;0;`"`"}),2,1)" '0'
Near 'GST extract, per-row blank rate' "INDEX($ge({110;110;110},{0.1;0;`"`"}),3,1)" '10'
Near 'GST add, per-row zero rate'      "INDEX($ga({100;100},{0;`"`"}),1,1)" '100'

# --- PAYG withholding, Schedule 1 for payments from 1 July 2026 (ATO, published 17 June 2026,
# read 16 November 2026): every row of the ATO's withholding amounts sample data, weekly,
# fortnightly and monthly, for scales 1, 2, 3, 5 and 6, and the ATO's 3 worked examples
# before their offsets and levy adjustments. The tables are the ATO's coefficients.
$pw = "oz.PAYGWithholding$L"
$pwScale = @{
    1 = '{188,0.15,0.15;371,0.2084,11.0185;515,0.179,0.1066;932,0.3227,74.1674;2246,0.32,71.6508;3303,0.39,228.8816;3303,0.47,493.1893}'
    2 = '{362,0,0;538,0.15,54.3462;673,0.25,108.2135;721,0.17,54.3473;865,0.179,60.8377;1282,0.3227,185.1935;2596,0.32,181.7319;3653,0.39,363.4627;3653,0.47,655.7704}'
    3 = '{2596,0.3,0.3;3653,0.37,181.7308;3653,0.45,474.0385}'
    5 = '{362,0,0;721,0.15,54.3462;865,0.159,60.8365;1282,0.3027,185.1923;2596,0.3,181.7308;3653,0.37,363.4615;3653,0.45,655.7692}'
    6 = '{362,0,0;721,0.15,54.3462;865,0.159,60.8365;908,0.3027,185.1923;1135,0.3527,230.6135;1282,0.3127,185.1923;2596,0.31,181.7308;3653,0.38,363.4615;3653,0.46,655.7692}'
}
# Period, earnings, then the amount for scales 1, 2, 3, 5 and 6.
$pwSample = @(
    'W 116 17 0 35 0 0',
    'W 117 18 0 35 0 0',
    'W 187 28 0 56 0 0',
    'W 188 28 0 56 0 0',
    'W 249 41 0 75 0 0',
    'W 250 41 0 75 0 0',
    'W 361 64 0 108 0 0',
    'W 362 65 0 109 0 0',
    'W 370 66 1 111 1 1',
    'W 371 66 1 111 1 1',
    'W 514 92 23 154 23 23',
    'W 515 92 23 154 23 23',
    'W 537 99 26 161 26 26',
    'W 538 100 27 161 27 27',
    'W 672 143 60 202 47 47',
    'W 673 143 60 202 47 47',
    'W 720 158 68 216 54 54',
    'W 721 159 68 216 54 54',
    'W 864 205 94 259 77 77',
    'W 865 205 94 259 77 77',
    'W 907 219 108 272 90 90',
    'W 908 219 108 272 90 90',
    'W 931 227 116 279 97 98',
    'W 932 227 116 280 97 98',
    'W 1134 292 181 340 158 170',
    'W 1135 292 181 340 159 170',
    'W 1281 339 229 384 203 216',
    'W 1282 339 229 385 203 216',
    'W 1844 519 409 553 372 390',
    'W 1845 519 409 553 372 391',
    'W 2119 607 497 636 454 475',
    'W 2120 607 497 636 455 476',
    'W 2245 647 537 673 492 515',
    'W 2246 647 537 674 492 515',
    'W 2490 743 615 747 566 590',
    'W 2491 743 616 747 566 591',
    'W 2595 784 649 778 597 623',
    'W 2596 784 649 779 597 623',
    'W 2652 806 671 800 618 645',
    'W 2653 806 672 800 619 645',
    'W 2736 839 704 831 649 677',
    'W 2737 839 704 831 650 677',
    'W 2898 902 767 891 709 738',
    'W 2899 902 768 891 710 739',
    'W 3302 1059 925 1040 859 892',
    'W 3303 1060 925 1041 859 892',
    'W 3652 1224 1061 1170 988 1025',
    'W 3653 1224 1062 1170 989 1025',
    'F 232 34 0 70 0 0',
    'F 234 36 0 70 0 0',
    'F 374 56 0 112 0 0',
    'F 376 56 0 112 0 0',
    'F 498 82 0 150 0 0',
    'F 500 82 0 150 0 0',
    'F 722 128 0 216 0 0',
    'F 724 130 0 218 0 0',
    'F 740 132 2 222 2 2',
    'F 742 132 2 222 2 2',
    'F 1028 184 46 308 46 46',
    'F 1030 184 46 308 46 46',
    'F 1074 198 52 322 52 52',
    'F 1076 200 54 322 54 54',
    'F 1344 286 120 404 94 94',
    'F 1346 286 120 404 94 94',
    'F 1440 316 136 432 108 108',
    'F 1442 318 136 432 108 108',
    'F 1728 410 188 518 154 154',
    'F 1730 410 188 518 154 154',
    'F 1814 438 216 544 180 180',
    'F 1816 438 216 544 180 180',
    'F 1862 454 232 558 194 196',
    'F 1864 454 232 560 194 196',
    'F 2268 584 362 680 316 340',
    'F 2270 584 362 680 318 340',
    'F 2562 678 458 768 406 432',
    'F 2564 678 458 770 406 432',
    'F 3688 1038 818 1106 744 780',
    'F 3690 1038 818 1106 744 782',
    'F 4238 1214 994 1272 908 950',
    'F 4240 1214 994 1272 910 952',
    'F 4490 1294 1074 1346 984 1030',
    'F 4492 1294 1074 1348 984 1030',
    'F 4980 1486 1230 1494 1132 1180',
    'F 4982 1486 1232 1494 1132 1182',
    'F 5190 1568 1298 1556 1194 1246',
    'F 5192 1568 1298 1558 1194 1246',
    'F 5304 1612 1342 1600 1236 1290',
    'F 5306 1612 1344 1600 1238 1290',
    'F 5472 1678 1408 1662 1298 1354',
    'F 5474 1678 1408 1662 1300 1354',
    'F 5796 1804 1534 1782 1418 1476',
    'F 5798 1804 1536 1782 1420 1478',
    'F 6604 2118 1850 2080 1718 1784',
    'F 6606 2120 1850 2082 1718 1784',
    'F 7304 2448 2122 2340 1976 2050',
    'F 7306 2448 2124 2340 1978 2050',
    'M 502.67 74 0 152 0 0',
    'M 507.00 78 0 152 0 0',
    'M 810.33 121 0 243 0 0',
    'M 814.67 121 0 243 0 0',
    'M 1079.00 178 0 325 0 0',
    'M 1083.33 178 0 325 0 0',
    'M 1564.33 277 0 468 0 0',
    'M 1568.67 282 0 472 0 0',
    'M 1603.33 286 4 481 4 4',
    'M 1607.67 286 4 481 4 4',
    'M 2227.33 399 100 667 100 100',
    'M 2231.67 399 100 667 100 100',
    'M 2327.00 429 113 698 113 113',
    'M 2331.33 433 117 698 117 117',
    'M 2912.00 620 260 875 204 204',
    'M 2916.33 620 260 875 204 204',
    'M 3120.00 685 295 936 234 234',
    'M 3124.33 689 295 936 234 234',
    'M 3744.00 888 407 1122 334 334',
    'M 3748.33 888 407 1122 334 334',
    'M 3930.33 949 468 1179 390 390',
    'M 3934.67 949 468 1179 390 390',
    'M 4034.33 984 503 1209 420 425',
    'M 4038.67 984 503 1213 420 425',
    'M 4914.00 1265 784 1473 685 737',
    'M 4918.33 1265 784 1473 689 737',
    'M 5551.00 1469 992 1664 880 936',
    'M 5555.33 1469 992 1668 880 936',
    'M 7990.67 2249 1772 2396 1612 1690',
    'M 7995.00 2249 1772 2396 1612 1694',
    'M 9182.33 2630 2154 2756 1967 2058',
    'M 9186.67 2630 2154 2756 1972 2063',
    'M 9728.33 2804 2327 2916 2132 2232',
    'M 9732.67 2804 2327 2921 2132 2232',
    'M 10790.00 3220 2665 3237 2453 2557',
    'M 10794.33 3220 2669 3237 2453 2561',
    'M 11245.00 3397 2812 3371 2587 2700',
    'M 11249.33 3397 2812 3376 2587 2700',
    'M 11492.00 3493 2908 3467 2678 2795',
    'M 11496.33 3493 2912 3467 2682 2795',
    'M 11856.00 3636 3051 3601 2812 2934',
    'M 11860.33 3636 3051 3601 2817 2934',
    'M 12558.00 3909 3324 3861 3072 3198',
    'M 12562.33 3909 3328 3861 3077 3202',
    'M 14308.67 4589 4008 4507 3722 3865',
    'M 14313.00 4593 4008 4511 3722 3865',
    'M 15825.33 5304 4598 5070 4281 4442',
    'M 15829.67 5304 4602 5070 4286 4442'
)
foreach ($row in $pwSample) {
    $f = -split $row
    $i = 2
    foreach ($scale in 1, 2, 3, 5, 6) {
        Near "PAYGWithholding: ATO sample $($f[0]) $($f[1]), scale $scale" "$pw($($f[1]), $($pwScale[$scale]), `"$($f[0])`")" $f[$i]
        $i++
    }
}
# The worked examples: Jabe, weekly 1,333.45 on scale 2; Prue, fortnightly 1,299.30 on
# scale 5; Tan, monthly 5,400.33 on scale 2, whose 33 cents gain a cent.
Near 'PAYGWithholding: ATO example, weekly'      "$pw(1333.45, $($pwScale[2]))"           '245'
Near 'PAYGWithholding: ATO example, fortnightly' "$pw(1299.30, $($pwScale[5]), `"F`")"   '86'
Near 'PAYGWithholding: ATO example, monthly'     "$pw(5400.33, $($pwScale[2]), `"m`")"   '940'
Near 'PAYGWithholding: a blank Period is weekly' "$pw(1333.45, $($pwScale[2]), Z1)"       '245'
# Quarterly: 13,000 / 13 is 1,000, so x is 1,000.99 and the week is 0.3227 * 1,000.99 - 185.1935 = 137.83, rounded to 138; 138 * 13.
Near 'PAYGWithholding: quarterly'                "$pw(13000, $($pwScale[2]), `"Q`")"     '1794'
# A column of payments gives a column, and a blank payment stays blank.
Same 'PAYGWithholding: a column of payments'     "TEXTJOIN(`",`",FALSE,$pw({116;1333.45;3653}, $($pwScale[2])))" '0,245,1062'
Same 'PAYGWithholding: a blank payment is blank' "$pw(Z1, $($pwScale[2]))" ''
$pwOne = "IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`")"
$pwTable = 'Coefficients must be 3 columns of numbers: weekly earnings less than, a and b, with the earnings rising and only the last row repeating the bound above it.'
$pwPaid = "PAYGWithholding$L needs each payment to be a number from 0 to under 10,000,000,000 in whole cents, or blank."
# A bound repeated above the last row would skip the row below it, and a fraction of a cent
# could tip the monthly 33-cent rule (17.329 a month reads as 17.34) or a weekly dollar. From
# 10,000,000,000 a double cannot show a fraction of a cent reliably: the nearest double to
# 46,000,000,000,000 and 1/128 is 0.21875 cents off a whole cent and passed the cents test. A
# small difference of very large sums carries noise above the allowance and is refused. 9E307
# * 100 overflows; that error must not pass as an error payment does. 9,999,999,999.99999 is
# under the bound and within the allowance, but its whole cents, 10,000,000,000.00, are not.
foreach ($r in @(@("1000, $($pwScale[2]), `"X`"", 'Period must be W, F, M or Q.'),
                 @("1000, {362,0;538,0.15}", $pwTable),
                 @("1000, {538,0.15,54.3462;362,0,0}", $pwTable),
                 @("1000, {362,0,`"0`"}", $pwTable),
                 @("538, {362,0,0;538,0.15,54.3462;538,0.25,108.2135;673,0.17,54.3473}", $pwTable),
                 @("-1, $($pwScale[2])", $pwPaid),
                 @("`"1000`", $($pwScale[2])", $pwPaid),
                 @("TRUE, $($pwScale[2])", $pwPaid),
                 @("17.329, $($pwScale[1]), `"M`"", $pwPaid),
                 @("{1000;1000.005}, $($pwScale[2])", $pwPaid),
                 @("537.9999, $($pwScale[1])", $pwPaid),
                 @("5000000000.005, $($pwScale[2])", $pwPaid),
                 @("46000000000000 + 1/128, $($pwScale[2])", $pwPaid),
                 @("10000000000, $($pwScale[2])", $pwPaid),
                 @("9999999999.99999, $($pwScale[2])", $pwPaid),
                 @("9E307, $($pwScale[2])", $pwPaid),
                 @("{1000;9E307}, $($pwScale[2])", $pwPaid),
                 @("REDUCE(5000000000, SEQUENCE(50, 1, 0.01, 0), LAMBDA(a, v, a + v)) - 5000000000, $($pwScale[2])", $pwPaid),
                 @("1000, {362,0,0;#N/A,0.15,54.3462}", $pwTable))) {
    Same "PAYGWithholding refuses ($($r[0]))" "LET(x, $pw($($r[0])), $pwOne)" $r[1]
}
# Only the last row may repeat the bound above it: a 2-row table whose rows share a bound is
# accepted, as is a 1-row table. A payment within a thousandth of a cent of a whole cent is that
# cent, so arithmetic noise is not taken for a fraction of a cent: 3 * 0.1 is not exactly 0.3 in
# binary, and 538 less a little is 538, so x is 538.99 and scale 1 gives
# 0.3227 * 538.99 - 74.1674 = 99.76, so 100. 10,000 additions of 12.34 one at a time leave
# 123,399.99999997; as 123,400.00, x is 123,400.99 and scale 2 gives
# 0.47 * 123,400.99 - 655.7704 = 57,342.69, so 57,343.
Near 'PAYGWithholding: a 2-row table may repeat its bound' "$pw(100, {200,0,0;200,0.1,0})" '0'
Near 'PAYGWithholding: a 1-row table'                     "$pw(100, {1000,0.1,0})"         '10'
Near 'PAYGWithholding: arithmetic in whole cents is accepted' "$pw(1333.15 + 3 * 0.1, $($pwScale[2]))" '245'
Near 'PAYGWithholding: 538 less arithmetic noise is 538'  "$pw(538 - 1E-13, $($pwScale[1]))"   '100'
Near 'PAYGWithholding: 538 less a billionth is 538'      "$pw(537.999999999, $($pwScale[1]))" '100'
Near 'PAYGWithholding: a long running sum of cents'      "$pw(REDUCE(0, SEQUENCE(10000, 1, 12.34, 0), LAMBDA(a, v, a + v)), $($pwScale[2]))" '57343'
Near 'PAYGWithholding: a large whole-cent amount'         "--ISNUMBER($pw(5000000000.02, $($pwScale[2])))" '1'
Near 'PAYGWithholding: the largest whole-cent amount'      "--ISNUMBER($pw(9999999999.99, $($pwScale[2])))" '1'
# A column keeps its shape: a number gives its amount, empty text stays empty and an error shows.
# The error is in an array constant: VSTACK returns #N/A whole when an argument is NA().
Same 'PAYGWithholding: a column of a number, empty text and an error' "LET(x, $pw({1333.45;`"`";#N/A}, $($pwScale[2])), INDEX(x,1,1) & `"|`" & INDEX(x,2,1) & `"|`" & ISNA(INDEX(x,3,1)))" '245||TRUE'
Near 'PAYGWithholding: an error in a payment shows' "--ISNA($pw(NA(), $($pwScale[2])))" '1'

# --- CGT parcels and net capital gain (ITAA 1997 ss 102-5, 115-25 and 115-100, compilation
# 264; ATO 'CGT discount' and TD 33, read 22 November 2026). The cases are those of
# australian-accounting's cgt-parcels command: the ATO's Kathleen and Boris examples, the
# Library's David Jones example and its hand-worked cases. Dates show as serial numbers:
# 45352 is 1 March 2024.
$cp = "oz.CGTParcels$L"
$ncg = "oz.NetCapitalGain$L"
$cpBody = "TEXTJOIN(`",`", FALSE, DROP(x, 1))"
$boris = "VSTACK(HSTACK(DATE(2024,3,1), 1000, 5000), HSTACK(DATE(2025,3,3), 3000, 30000)), HSTACK(DATE(2026,3,2), 1500, 12000)"
Same 'CGTParcels: Boris, first in, first out' "LET(x, $cp($boris), $cpBody)" '46083,45352,1000,8000,5000,3000,0,TRUE,46083,45719,500,4000,5000,0,1000,FALSE'
Same 'CGTParcels: the header row' "TEXTJOIN(`",`", FALSE, TAKE($cp($boris), 1))" 'Sold,Acquired,Units,Proceeds,Cost base,Gain,Loss,12 months'
Near 'NetCapitalGain: Boris from the parcels' "LET(t, DROP($cp($boris), 1), INDEX($ncg(2026, CHOOSECOLS(t, 6), CHOOSECOLS(t, 8), CHOOSECOLS(t, 7)), 2, 1))" '1000' '0.005'
# Kathleen: one call a company, then her $400 of earlier losses
Same 'CGTParcels: Kathleen, Company A' "LET(x, $cp(HSTACK(DATE(2000,11,17), 1000, 3000), HSTACK(DATE(2025,12,17), 1000, 6000)), $cpBody)" '46008,36847,1000,6000,3000,3000,0,TRUE'
Same 'CGTParcels: Kathleen, Company B' "LET(x, $cp(HSTACK(DATE(2025,10,10), 130, 520), HSTACK(DATE(2026,2,27), 130, 1040)), $cpBody)" '46080,45940,130,1040,520,520,0,FALSE'
Same 'CGTParcels: Kathleen, Company C' "LET(x, $cp(HSTACK(DATE(2025,10,10), 600, 2400), HSTACK(DATE(2026,6,25), 600, 1800)), $cpBody)" '46198,45940,600,1800,2400,0,600,FALSE'
Same 'NetCapitalGain: Kathleen' "TEXTJOIN(`",`", FALSE, $ncg(2026, {3000;520}, {TRUE;FALSE}, 600, 400))" 'Net capital gain,Discount,Losses carried forward,1260,1260,0'
# The 12 months count neither the day acquired nor the day of the sale; from 29 February
# they end on 1 March.
foreach ($r in @(@('DATE(2025,2,2)', 'DATE(2026,2,3)', 'TRUE'),
                 @('DATE(2025,2,2)', 'DATE(2026,2,2)', 'FALSE'),
                 @('DATE(2024,2,28)', 'DATE(2025,2,28)', 'FALSE'),
                 @('DATE(2024,2,28)', 'DATE(2025,3,1)', 'TRUE'),
                 @('DATE(2024,2,29)', 'DATE(2025,2,28)', 'FALSE'),
                 @('DATE(2024,2,29)', 'DATE(2025,3,1)', 'TRUE'))) {
    Same "CGTParcels: bought $($r[0]), sold $($r[1])" "INDEX($cp(HSTACK($($r[0]), 1, 800), HSTACK($($r[1]), 1, 1000)), 2, 8) & `"`"" $r[2]
}
# Each split rounds to the cent and the shares add up to the whole
Same 'CGTParcels: a parcel split across 2 sales' "LET(x, $cp(HSTACK(DATE(2025,8,1), 3, 110), VSTACK(HSTACK(DATE(2025,9,1), 1, 50), HSTACK(DATE(2025,10,1), 2, 100))), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 5, 6)))" '36.67,13.33,73.33,26.67'
Same 'CGTParcels: a sale split across 2 parcels' "LET(x, $cp(VSTACK(HSTACK(DATE(2025,8,1), 1, 30), HSTACK(DATE(2025,8,2), 2, 60)), HSTACK(DATE(2025,9,1), 3, 100)), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 4)))" '33.33,66.67'
Same 'CGTParcels: parcels and sales out of date order' "LET(x, $cp(VSTACK(HSTACK(DATE(2025,8,2), 100, 2000), HSTACK(DATE(2025,8,1), 100, 1000)), VSTACK(HSTACK(DATE(2025,10,1), 50, 900), HSTACK(DATE(2025,9,1), 50, 600), HSTACK(DATE(2025,9,15), 100, 2000))), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 3, 5)))" '50,500,50,500,50,1000,50,1000'
Same 'CGTParcels: a sale before a later parcel' "LET(x, $cp(VSTACK(HSTACK(DATE(2025,8,1), 10, 100), HSTACK(DATE(2025,9,10), 10, 200)), VSTACK(HSTACK(DATE(2025,9,1), 10, 150), HSTACK(DATE(2025,9,20), 10, 150))), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 2, 6, 7)))" '45870,50,0,45910,0,50'
Same 'CGTParcels: units in decimals' "LET(x, $cp(HSTACK(DATE(2025,8,1), 10.5, 105), VSTACK(HSTACK(DATE(2025,9,1), 0.1, 2), HSTACK(DATE(2025,9,2), 0.2, 3))), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 3, 5)))" '0.1,1,0.2,2'
Same 'CGTParcels: same-day parcels keep the order given' "LET(x, $cp(VSTACK(HSTACK(DATE(2025,8,1), 1, 10), HSTACK(DATE(2025,8,1), 1, 20)), HSTACK(DATE(2025,9,1), 1, 15)), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 5)))" '10'
Same 'CGTParcels: net proceeds below 0' "LET(x, $cp(HSTACK(DATE(2025,8,1), 10, 100), HSTACK(DATE(2025,9,1), 10, -10)), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 6, 7)))" '0,110'
Same 'CGTParcels: a parcel from 22 September 1999' "LET(x, $cp(HSTACK(DATE(1999,9,22), 1, 10), HSTACK(DATE(2027,6,30), 1, 15)), $cpBody)" '46568,36425,1,15,10,5,0,TRUE'
# Spans are counted in whole millionths: a millionth of a unit bought after 999,999,998 units
# keeps its own cost and proceeds; and 1,000 parcels against 1,000 sales, a million pairs,
# fit the column TOCOL builds, as do 1,001 weekly parcels sold together. A share of exactly
# half a cent rounds away from zero: half of 495,372,740,747 cents is 247,686,370,373.5.
$big = "VSTACK(HSTACK(DATE(2025,8,1), 999999998, 1000), HSTACK(DATE(2025,8,2), 0.000001, 100)), VSTACK(HSTACK(DATE(2025,9,1), 999999998, 1000), HSTACK(DATE(2025,9,2), 0.000001, 120))"
Same 'CGTParcels: a millionth of a unit after 999,999,998' "LET(x, DROP($cp($big), 2), TEXTJOIN(`",`", FALSE, CHOOSECOLS(x, 3) * 1000000, CHOOSECOLS(x, 4), CHOOSECOLS(x, 5), CHOOSECOLS(x, 6)))" '1,120,100,20'
Same 'CGTParcels: 1,000 parcels against 1,000 sales' "LET(x, $cp(HSTACK(SEQUENCE(1000,1,45870,0), SEQUENCE(1000,1,1,0), SEQUENCE(1000,1,1,0)), HSTACK(SEQUENCE(1000,1,45901,0), SEQUENCE(1000,1,1,0), SEQUENCE(1000,1,2,0))), ROWS(x) & `"|`" & SUM(CHOOSECOLS(DROP(x, 1), 6)))" '1001|1000'
Same 'CGTParcels: 1,001 parcels sold together' "LET(x, $cp(HSTACK(SEQUENCE(1001,1,38870,7), SEQUENCE(1001,1,1,0), SEQUENCE(1001,1,1,0)), HSTACK(DATE(2026,1,1), 1001, 2002)), ROWS(x) & `"|`" & SUM(CHOOSECOLS(DROP(x, 1), 6)))" '1002|1001'
Same 'CGTParcels: 16,385 sales of one parcel' "LET(x, $cp({45870,20000,20000}, HSTACK(SEQUENCE(16385,1,45901,0), SEQUENCE(16385,1,1,0), SEQUENCE(16385,1,3,0))), ROWS(x) & `"|`" & SUM(CHOOSECOLS(DROP(x, 1), 6)))" '16386|32770'
Same 'CGTParcels: 16,385 parcels in one sale' "LET(x, $cp(HSTACK(SEQUENCE(16385,1,45870,0), SEQUENCE(16385,1,1,0), SEQUENCE(16385,1,1,0)), {45901,16385,49155}), ROWS(x) & `"|`" & SUM(CHOOSECOLS(DROP(x, 1), 6)))" '16386|32770'
Same 'CGTParcels: a half-cent share rounds up' "LET(x, $cp(HSTACK(DATE(2025,8,1), 900000000, 4953727407.47), VSTACK(HSTACK(DATE(2025,9,1), 450000000, 1), HSTACK(DATE(2025,9,2), 450000000, 1))), TEXTJOIN(`",`", FALSE, CHOOSECOLS(DROP(x, 1), 5)))" '2476863703.74,2476863703.73'
$cpTable = "Parcels and Sales must each be 3 columns of numbers, with at most 1,000,000 parcel and sale pairs: the date; the units, in millionths from 0.000001 and under 1,000,000,000 in all; and the parcel's cost base, 0 or more, or the sale's proceeds less its selling costs, in whole cents under 10,000,000,000."
$cpCover = "CGTParcels$L refuses a sale of more units than the parcels acquired by its date."
$cpLate = "CGTParcels$L works sales before 1 July 2027: the Treasury Laws Amendment (Tax Reform No. 1) Act 2026 changes CGT from that day."
$cpOld = "CGTParcels$L refuses a sale taking units from a parcel acquired on or before 21 September 1999: indexation or the pre-CGT rules can apply."
$one = "IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`")"
foreach ($r in @(@('{45870,10}, {45901,1,10}', $cpTable),
                 @('{45870,10,100}, {45901,1}', $cpTable),
                 @('{45870,0,100}, {45901,1,10}', $cpTable),
                 @('{45870,10,-1}, {45901,1,10}', $cpTable),
                 @('{45870,0.0000001,100}, {45901,1,10}', $cpTable),
                 @('{45870.5,10,100}, {45901,1,10}', $cpTable),
                 @('{45870,10,"100"}, {45901,1,10}', $cpTable),
                 @('{45870,10,100}, {45901,1,#N/A}', $cpTable),
                 @('{45870,0.0000000001,100;45871,1,200}, {45901,1,300}', $cpTable),
                 @('{45870,999999999,1;45871,1,1}, {45901,1,1}', $cpTable),
                 @('{45870,1,1.007}, {45901,1,2}', $cpTable),
                 @('{45870,1,1}, {45901,1,2.003}', $cpTable),
                 @('{45870,1,10000000000}, {45901,1,2}', $cpTable),
                 @('HSTACK(SEQUENCE(1001,1,45870,0), SEQUENCE(1001,1,1,0), SEQUENCE(1001,1,1,0)), HSTACK(SEQUENCE(1000,1,45901,0), SEQUENCE(1000,1,1,0), SEQUENCE(1000,1,1,0))', $cpTable),
                 @('{45870,10,100}, {45901,11,10}', $cpCover),
                 @('{45901,10,100}, {45870,1,10}', $cpCover),
                 @('{45870,10,100}, {46569,1,10}', $cpLate),
                 @('{36424,10,100}, {45901,1,10}', $cpOld))) {
    Same "CGTParcels refuses ($($r[0]))" "LET(x, $cp($($r[0])), $one)" $r[1]
}
# NetCapitalGain: the discount percentage follows the entity, losses go to the other gains
# first, and what is left of the losses carries forward.
foreach ($r in @(@('2026, 289755, TRUE, , , "S"', '193170,96585,0'),
                 @('2026, 289755, TRUE, , , "t"', '144877.5,144877.5,0'),
                 @('2027, 289755, TRUE, , , "C"', '289755,0,0'),
                 @('2027, 150, , 100, 20, "C"', '30,0,0'),
                 @('2026, {8000;21435}, {FALSE;TRUE}, 8660', '10387.5,10387.5,0'),
                 @('2026, {8000;21435}, {0;1}, {2000;6660}', '10387.5,10387.5,0'),
                 @('2026, 100, TRUE, 300, 50', '0,0,250'),
                 @('2026, 100, , 30', '70,0,0'),
                 @('2026, {100;""}, {TRUE;""}, , , ""', '50,50,0'),
                 @('2027, 0', '0,0,0'),
                 @('2026, {9999999999.99;0.01}, , 9999999999.99', '0.01,0,0'),
                 @('2026, SEQUENCE(1000,1,9999999999.99,0), SEQUENCE(1000,1,1,0), SEQUENCE(1000,1,9999999999.99,0)', '0,0,0'))) {
    Same "NetCapitalGain: ($($r[0]))" "TEXTJOIN(`",`", FALSE, DROP($ncg($($r[0])), 1))" $r[1]
}
$ncgYear = 'IncomeYear must be 2026 for 2025-26 or 2027 for 2026-27: the Treasury Laws Amendment (Tax Reform No. 1) Act 2026 changes the method from 2027-28.'
$ncgEntity = 'Entity must be I (individual), T (trust), S (complying superannuation entity) or C (company).'
$ncgAmount = 'Gains, Losses and PriorLosses must be whole cents from 0 to under 10,000,000,000, or blank, with the gains, and the losses, each under 10,000,000,000,000 in all.'
$ncgFlag = 'Discountable must match Gains cell for cell, each TRUE, FALSE, 1, 0 or blank.'
foreach ($r in @(@('2028, 100', $ncgYear),
                 @('2025, 100', $ncgYear),
                 @('"2026", 100', $ncgYear),
                 @('{2026;2027}, 100', $ncgYear),
                 @('2026, 100, , , , "X"', $ncgEntity),
                 @('2026, -1', $ncgAmount),
                 @('2026, "100"', $ncgAmount),
                 @('2026, 100, , -5', $ncgAmount),
                 @('2026, 100, , , #N/A', $ncgAmount),
                 @('2026, {100;#N/A}, {TRUE;TRUE}', $ncgAmount),
                 @('2026, 100.005', $ncgAmount),
                 @('2026, 10000000000', $ncgAmount),
                 @('2026, SEQUENCE(1001,1,9999999999.99,0)', $ncgAmount),
                 @('2026, {100;200}, TRUE', $ncgFlag),
                 @('2026, 100, 2', $ncgFlag),
                 @('2026, 100, "yes"', $ncgFlag))) {
    Same "NetCapitalGain refuses ($($r[0]))" "LET(x, $ncg($($r[0])), $one)" $r[1]
}

# --- Income tax on taxable income (Income Tax Rates Act 1986 Schedule 7 Parts I and II,
# compilation 66; the ATO's resident and foreign resident rate tables, read 22 November 2026).
# The base amounts are the ATO's; the other cases are those of australian-accounting's
# resident-tax and foreign-resident-tax commands.
$it = "oz.IncomeTax$L"
foreach ($y in @('2025', '2026')) {
    foreach ($r in @(@('0', '0'), @('18200', '0'), @('18201', '0.16'), @('45000', '4288'), @('45001', '4288.3'),
                     @('135000', '31288'), @('135001', '31288.37'), @('190000', '51638'), @('190001', '51638.45'))) {
        Near "IncomeTax: resident $($r[0]) in $y" "$it($($r[0]), $y)" $r[1] '0.000001'
    }
}
foreach ($r in @(@('18200', '0'), @('18201', '0.15'), @('45000', '4020'), @('135000', '31020'), @('190000', '51370'),
                 @('200000', '55870'))) {
    Near "IncomeTax: resident $($r[0]) in 2027" "$it($($r[0]), 2027)" $r[1] '0.000001'
}
foreach ($y in @('2025', '2026', '2027')) {
    foreach ($r in @(@('0', '0'), @('1', '0.3'), @('18200', '5460'), @('134999', '40499.7'), @('135000', '40500'),
                     @('135001', '40500.37'), @('189999', '60849.63'), @('190000', '60850'), @('190001', '60850.45'))) {
        Near "IncomeTax: foreign resident $($r[0]) in $y" "$it($($r[0]), $y, FALSE)" $r[1] '0.000001'
    }
}
Near 'IncomeTax: 31,130 in 2025-26'           "$it(31130, 2026)"    '2068.8' '0.000001'
Near 'IncomeTax: 90,000 in 2026-27'           "$it(90000, 2027)"    '17520' '0.000001'
Near 'IncomeTax: 0 is a foreign resident'     "$it(50000, 2026, 0)" '15000' '0.000001'
Near 'IncomeTax: 1 is a resident'             "$it(50000, 2026, 1)" '5788' '0.000001'
Near 'IncomeTax: a blank Resident is resident' "$it(50000, 2026, Z1)" '5788' '0.000001'
Near 'IncomeTax: cents are taxed'             "$it(18200.5, 2026)"  '0.08' '0.000001'
foreach ($r in @(@('18200.01', '0.0016'), @('45000.01', '4288.003'), @('135000.01', '31288.0037'), @('190000.01', '51638.0045'))) {
    Near "IncomeTax: a cent above $($r[0])" "$it($($r[0]), 2026)" $r[1] '0.0000001'
}
Near 'IncomeTax: a foreign resident a cent above 135,000' "$it(135000.01, 2026, FALSE)" '40500.0037' '0.0000001'
Same 'IncomeTax: blank cells stay blank'      "TEXTJOIN(`",`", FALSE, $it(Z1:Z3, 2026))" ',,'
Same 'IncomeTax: a column with an error'      "LET(x, $it({0;#N/A;18201}, 2026), INDEX(x,1,1) & `"|`" & ISNA(INDEX(x,2,1)) & `"|`" & INDEX(x,3,1))" '0|TRUE|0.16'
Same 'IncomeTax: a column with a blank'       "TEXTJOIN(`",`", FALSE, $it({0;18201;`"`";45000}, 2026))" '0,0.16,,4288'
Same 'IncomeTax: a row keeps its shape'       "COLUMNS($it({45000,90000}, 2027)) & `"|`" & INDEX($it({45000,90000}, 2027), 1, 2)" '2|17520'
Near 'IncomeTax: an error shows'              "--ISNA($it(NA(), 2026))" '1'
$itYear = 'IncomeYear must be 2025, 2026 or 2027, the year the income year ends; 2027-28 and later are not supported.'
$itHome = 'Resident must be TRUE or 1 for a resident, FALSE or 0 for a foreign resident.'
$itIncome = "IncomeTax$L needs each taxable income to be a number from 0 to under 10,000,000,000,000, or blank."
foreach ($r in @(@('1000, 2024', $itYear),
                 @('1000, 2028', $itYear),
                 @('1000, "2026"', $itYear),
                 @('1000, {2025;2026}', $itYear),
                 @('1000, 2026, 2', $itHome),
                 @('1000, 2026, "yes"', $itHome),
                 @('1000, 2026, {TRUE;FALSE}', $itHome),
                 @('1000, 2026, " "', $itHome),
                 @('1000, DATE(2026,6,30)', $itYear),
                 @('" ", 2026', $itIncome),
                 @('-1, 2026', $itIncome),
                 @('"1000", 2026', $itIncome),
                 @('TRUE, 2026', $itIncome),
                 @('1E13, 2026', $itIncome))) {
    Same "IncomeTax refuses ($($r[0]))" "LET(x, $it($($r[0])), $one)" $r[1]
}

# --- Division 7A minimum yearly repayment (ITAA 1936 s 109E(6), compilation 193; the ATO's
# benchmark rate page and 'Loans by private companies', read 22 November 2026). The ATO's 2
# worked examples, then the cases of australian-accounting's div7a minimum-yearly-repayment
# command within a 25-year remaining term, each rounded there to the cent.
$d7 = "oz.Div7AMinRepayment$L"
Near 'Div7A: the ATO 2015 example, $9,835' "ROUND($d7(55000, 5.95%, 7), 0)" '9835'
Near 'Div7A: the ATO 2016 example, $10,079' "ROUND($d7(50430, 5.45%, 6), 0)" '10079'
foreach ($r in @(@('10000, 0.0877, 1', '10877'), @('20000, 0.0877, 7', '3943.19'), @('60000, 0.0877, 6', '13283.6'),
                 @('100000, 0.0877, 5', '25556'), @('120000, 0.0877, 3', '47212.37'),
                 @('68354493386.42, 0.0877, 3', '26893146759.85'), @('209701339756.06, 0.0837, 4', '63835421599.71'),
                 @('494255592778.05, 0.0837, 8', '87219223754.29'), @('575073514054.77, 0.0837, 4', '175058777703.66'),
                 @('1000000000000, 0.0877, 22', '104073401943.45'), @('1000000000000, 0.0877, 4', '307111815954.34'),
                 @('0, 0.0877, 1', '0'), @('0, 0.0877, 25', '0'))) {
    Near "Div7A: australian-accounting ($($r[0]))" "$d7($($r[0]))" $r[1] '0.005'
}
Near 'Div7A: a part year rounds up'           "$d7(100000, 8.77%, 4.2) - $d7(100000, 8.77%, 5)" '0'
Near 'Div7A: a sliver over 4 years is 5'      "$d7(100000, 8.77%, 4.000001) - $d7(100000, 8.77%, 5)" '0'
Near 'Div7A: 25 years is the longest'         "$d7(100000, 8.77%, 25)" '9991.528865' '0.00001'
Near 'Div7A: 1 year is the balance with interest' "$d7(100000, 8.37%, 1)" '108370' '0.000001'
Same 'Div7A: one term for a column with a blank' "LET(x, $d7({100000;`"`";60000}, 8.77%, {5;`"`";6}), TEXTJOIN(`",`", FALSE, IF(x = `"`", `"`", ROUND(x, 2))))" '25556,,13283.6'
Same 'Div7A: one term for every balance'      "TEXTJOIN(`",`", FALSE, ROUND($d7({100000;60000}, 8.77%, 5), 2))" '25556,15333.6'
Same 'Div7A: a row keeps its shape'           "COLUMNS($d7({100000,60000}, 8.77%, {5,6})) & `"|`" & ROUND(INDEX($d7({100000,60000}, 8.77%, {5,6}), 1, 2), 2)" '2|13283.6'
Same 'Div7A: a blank balance stays blank'     "$d7(Z1, 8.77%, 5)" ''
Near 'Div7A: an error balance shows'          "--ISNA($d7(NA(), 8.77%, 5))" '1'
Near 'Div7A: an error term shows'             "--ISNA($d7(100000, 8.77%, NA()))" '1'
Near 'Div7A: an error rate shows'             "--ISNA($d7(100000, NA(), 5))" '1'
Near 'Div7A: a rate of 1E-16 stays stable'    "$d7(1000, 1E-16, 25)" '40' '0.000000001'
$d7Rate = 'BenchmarkRate must be one rate above 0 and below 1, such as 8.77% or 0.0877.'
$d7Shape = 'RemainingTerm must be one term, or a term for each opening balance in the same shape.'
$d7Balance = "Div7AMinRepayment$L needs each opening balance to be a number from 0 to under 10,000,000,000,000, or blank."
$d7Term = 'RemainingTerm must be more than 0 and at most 25 years for each opening balance.'
foreach ($r in @(@('1000, 0, 5', $d7Rate),
                 @('1000, 1, 5', $d7Rate),
                 @('1000, 8.77, 5', $d7Rate),
                 @('1000, "8.77%", 5', $d7Rate),
                 @('1000, {0.08;0.09}, 5', $d7Rate),
                 @('1000, Z1, 5', $d7Rate),
                 @('{1000;2000}, 8.77%, {5;6;7}', $d7Shape),
                 @('{1000;2000}, 8.77%, {5,6}', $d7Shape),
                 @('-1, 8.77%, 5', $d7Balance),
                 @('"1000", 8.77%, 5', $d7Balance),
                 @('" ", 8.77%, 5', $d7Balance),
                 @('TRUE, 8.77%, 5', $d7Balance),
                 @('1E13, 8.77%, 5', $d7Balance),
                 @('1000, 8.77%, 0', $d7Term),
                 @('1000, 8.77%, -1', $d7Term),
                 @('1000, 8.77%, 25.000001', $d7Term),
                 @('1000, 8.77%, 26', $d7Term),
                 @('1000, 8.77%, "5"', $d7Term),
                 @('1000, 8.77%, TRUE', $d7Term),
                 @('1000, 8.77%, Z1', $d7Term),
                 @('{1000;2000}, 8.77%, {5;""}', $d7Term))) {
    Same "Div7A refuses ($($r[0]))" "LET(x, $d7($($r[0])), $one)" $r[1]
}

# --- Margin scheme GST (GST Act s 75-10, compilation 96; the ATO's 'Calculating the GST
# payable', read 22 November 2026). The ATO's 3 worked examples, then the cases of
# australian-accounting's gst-margin-scheme command, which rounds to the cent under s 9-90(1).
$ms = "oz.MarginSchemeGST$L"
Near 'MarginSchemeGST: the ATO John example, $36,363' "INT($ms(900000, 500000))" '36363'
Near 'MarginSchemeGST: the ATO Diane example, $20,000' "$ms(720000, 500000)" '20000'
Near 'MarginSchemeGST: the ATO Jamie example, valuation' "$ms(880000, 330000)" '50000'
foreach ($r in @(@('640000, 448000', '17454.55'), @('144000, 100000', '4000'), @('144000, 75000', '6272.73'),
                 @('360000, 250000', '10000'), @('155000, 100000', '5000'), @('166000, 100000', '6000'),
                 @('300000, 166666.67', '12121.21'), @('610000, 100000', '46363.64'),
                 @('1500000, 250000', '113636.36'), @('900000, 500000', '36363.64'),
                 @('1100000, 220000', '80000'), @('200000, 200000', '0'), @('180000, 200000', '0'))) {
    Near "MarginSchemeGST: australian-accounting ($($r[0]))" "ROUND($ms($($r[0])), 2)" $r[1]
}
Near 'MarginSchemeGST: unrounded'                 "$ms(640000, 448000)" '17454.5454545' '0.0000001'
Near 'MarginSchemeGST: a nil base'                "$ms(1100, 0)" '100'
Same 'MarginSchemeGST: a column with a blank'     "LET(x, $ms({640000;`"`";180000}, {448000;`"`";200000}), TEXTJOIN(`",`", FALSE, IF(x = `"`", `"`", ROUND(x, 2))))" '17454.55,,0'
Same 'MarginSchemeGST: a row keeps its shape'     "COLUMNS($ms({720000,880000}, {500000,330000})) & `"|`" & INDEX($ms({720000,880000}, {500000,330000}), 1, 2)" '2|50000'
Same 'MarginSchemeGST: a blank price stays blank' "$ms(Z1, 100)" ''
Same 'MarginSchemeGST: a blank price needs no base' "$ms(Z1, Z1)" ''
Near 'MarginSchemeGST: an error price shows'      "--ISNA($ms(NA(), 100))" '1'
Near 'MarginSchemeGST: an error base shows'       "--ISNA($ms(100, NA()))" '1'
$msShape = 'MarginBase must have one base for each consideration, in the same shape.'
$msPrice = "MarginSchemeGST$L needs each consideration to be a number from 0 to under 10,000,000,000,000, or blank."
$msBase = 'MarginBase must be a number from 0 to under 10,000,000,000,000 beside each consideration.'
foreach ($r in @(@('{1000;2000}, 100', $msShape),
                 @('{1000;2000}, {100,200}', $msShape),
                 @('-1, 100', $msPrice),
                 @('"1000", 100', $msPrice),
                 @('" ", 100', $msPrice),
                 @('TRUE, 100', $msPrice),
                 @('1E13, 100', $msPrice),
                 @('1000, -1', $msBase),
                 @('1000, "100"', $msBase),
                 @('1000, TRUE', $msBase),
                 @('1000, 1E13', $msBase),
                 @('1000, Z1', $msBase),
                 @('{1000;2000}, {100;""}', $msBase))) {
    Same "MarginSchemeGST refuses ($($r[0]))" "LET(x, $ms($($r[0])), $one)" $r[1]
}

# --- FBT gross-up (FBTAA s 5B(1B) and (1C), compilation 97; Fringe Benefits Tax Act 1986 s 6;
# the ATO's FBT rates and thresholds page, read 22 November 2026). The ATO's published factors
# and reportable example, then the cases of australian-accounting's fbt command, which uses
# the statute's factors and rounds to the cent.
$fg = "oz.FBTGrossUp$L"
Near 'FBTGrossUp: type 1 factor is the ATO 2.0802' "ROUND($fg(1, 1), 4)" '2.0802'
Near 'FBTGrossUp: type 2 factor is the ATO 1.8868' "ROUND($fg(1, 2), 4)" '1.8868'
Near 'FBTGrossUp: type 1 factor = 57000/27401'    "$fg(1, 1)" '2.08021605050910' '0.0000000000001'
Near 'FBTGrossUp: type 2 factor = 100/53'         "$fg(1, 2)" '1.88679245283019' '0.0000000000001'
Near 'FBTGrossUp: the ATO reportable $2,000.01 example, $3,773' "INT($fg(2000.01, 2))" '3773'
foreach ($r in @(@('16500, 1', '34323.56'), @('6000, 2', '11320.75'), @('11000, 1', '22882.38'), @('9000, 2', '16981.13'),
                 @('10000, 2', '18867.92'), @('1000000, 1', '2080216.05'), @('1000000, 2', '1886792.45'))) {
    Near "FBTGrossUp: australian-accounting ($($r[0]))" "ROUND($fg($($r[0])), 2)" $r[1]
}
Near 'FBTGrossUp: nil is nil'                     "$fg(0, 1)" '0'
Same 'FBTGrossUp: one type for a column with a blank' "LET(x, $fg({16500;`"`";6000}, 2), TEXTJOIN(`",`", FALSE, IF(x = `"`", `"`", ROUND(x, 2))))" '31132.08,,11320.75'
Same 'FBTGrossUp: a type for each'               "TEXTJOIN(`",`", FALSE, ROUND($fg({16500;6000}, {1;2}), 2))" '34323.56,11320.75'
Same 'FBTGrossUp: a row keeps its shape'          "COLUMNS($fg({16500,6000}, {1,2})) & `"|`" & ROUND(INDEX($fg({16500,6000}, {1,2}), 1, 2), 2)" '2|11320.75'
Same 'FBTGrossUp: a blank value stays blank'      "$fg(Z1, 1)" ''
Same 'FBTGrossUp: a blank value needs no type'    "LET(x, $fg({16500;`"`"}, {1;`"`"}), TEXTJOIN(`",`", FALSE, IF(x = `"`", `"`", ROUND(x, 2))))" '34323.56,'
Near 'FBTGrossUp: an error value shows'           "--ISNA($fg(NA(), 1))" '1'
Near 'FBTGrossUp: an error type shows'            "--ISNA($fg(100, NA()))" '1'
$fgShape = 'Type must be one type, or a type for each taxable value in the same shape.'
$fgValue = "FBTGrossUp$L needs each taxable value to be a number from 0 to under 10,000,000,000,000, or blank."
$fgType = 'Type must be 1 or 2 for each taxable value.'
foreach ($r in @(@('{1000;2000}, {1;2;1}', $fgShape),
                 @('{1000;2000}, {1,2}', $fgShape),
                 @('-1, 1', $fgValue),
                 @('"1000", 1', $fgValue),
                 @('" ", 1', $fgValue),
                 @('TRUE, 1', $fgValue),
                 @('1E13, 1', $fgValue),
                 @('1000, 0', $fgType),
                 @('1000, 3', $fgType),
                 @('1000, 1.5', $fgType),
                 @('1000, "1"', $fgType),
                 @('1000, TRUE', $fgType),
                 @('1000, Z1', $fgType),
                 @('{1000;2000}, {1;""}', $fgType))) {
    Same "FBTGrossUp refuses ($($r[0]))" "LET(x, $fg($($r[0])), $one)" $r[1]
}

# --- Depreciation pools (ITAA 1997 compilation 268: s 40-440 and s 40-445; Subdivision 328-D,
# s 328-190 to s 328-215; IT(TP)A 1997 compilation 104 s 328-180(6)(e); read 22 November 2026).
# Rows: opening, deduction, closing, assessable; one column per income year. The cases of
# australian-accounting's depreciation_low_value_pool and depreciation_small_business_pool
# commands (commit 2ef04d2), which round each step to the cent, so results are compared to 2
# places. That command rounds the decline before the closing balance; this result does not,
# so its first closing balance differs by half a cent.
$lv = "oz.LowValuePool$L"
$sb = "oz.SmallBusinessPool$L"
$rows = "TEXTJOIN(`",`", FALSE, ROUND(x, 2))"
foreach ($r in @(@('3608, 0, 0, 300', '3608,1353,1955,0'),
                 @('1000, 400, 800', '1000,750,1450,0'),
                 @('0.01, 0.02', '0.01,0.01,0.02,0'),
                 @('1000, 0, , 1000', '1000,375,0,375'),
                 @('5000, {594,0}', '5000,3607.63,1986.38,1352.86,3607.63,2254.77,0,0'))) {
    Same "LowValuePool: ($($r[0]))" "LET(x, $lv($($r[0])), $rows)" $r[1]
}
Near 'LowValuePool: australian-accounting 5000 and 594 closes at 3607.62' "INDEX($lv(5000, 594), 3, 1)" '3607.62' '0.0051'
Near 'LowValuePool: decline is 18.75% and 37.5%' "INDEX($lv(1000, 400, 800), 2, 1)" '750'
Same 'LowValuePool: each year opens at the last close' "LET(x, $lv(1000, {400,0,0}, {800,0,0}, {0,100,0}), TEXTJOIN(`",`", FALSE, ROUND(CHOOSEROWS(x, 1), 4)) & `"|`" & TEXTJOIN(`",`", FALSE, ROUND(DROP(CHOOSEROWS(x, 3), , -1), 4)))" '1000,1450,806.25|1450,806.25'
Same 'LowValuePool: termination values above the pool are assessable' "LET(x, $lv(100, 0, 0, {1000}), $rows)" '100,37.5,0,937.5'
Same 'LowValuePool: one value for every year' "LET(x, $lv(0, {100,100}, 50), $rows)" '0,112.5,37.5,79.69,112.5,182.81,0,0'
Same 'LowValuePool: a blank opening is 0' "LET(x, $lv(Z1, 100), $rows)" '0,18.75,81.25,0'
Same 'LowValuePool: a blank amount is 0' "LET(x, $lv(100, Z1), $rows)" '100,37.5,62.5,0'
Near 'LowValuePool: an error opening shows' "--ISNA(INDEX($lv(NA(), 100), 1, 1))" '1'
Near 'LowValuePool: an error amount shows' "--ISNA(INDEX($lv(100, NA()), 2, 1))" '1'
Same 'LowValuePool: four rows and a column for each year' "ROWS($lv(0, {1,2,3})) & `"x`" & COLUMNS($lv(0, {1,2,3}))" '4x3'
$lvShape = 'OpeningBalance must be one value, LowCostAssets one row with a value for each income year, and LowValueAssets and TerminationValues one value or one for each year.'
$lvValue = "LowValuePool$L needs the opening balance and each amount to be a number from 0 to under 10,000,000,000,000, or blank."
foreach ($r in @(@('{1,2}, 100', $lvShape), @('100, {1;2}', $lvShape), @('100, {1,2}, {1,2,3}', $lvShape),
                 @('100, {1,2}, , {1;2}', $lvShape), @('-1, 100', $lvValue), @('100, -1', $lvValue),
                 @('100, 1, "1"', $lvValue), @('100, 1, , TRUE', $lvValue), @('1E13, 1', $lvValue), @('" ", 1', $lvValue))) {
    Same "LowValuePool refuses ($($r[0]))" "LET(x, $lv($($r[0])), $one)" $r[1]
}
foreach ($r in @(@('100000, 28000, 0, 8000', '100000,34200,85800,0'),
                 @('28500, 16800, , 29600', '28500,15700,0,0'),
                 @('1000000, 0', '1000000,300000,700000,0'),
                 @('20000, 0', '20000,6000,14000,0'),
                 @('19999.99, 0', '19999.99,19999.99,0,0'),
                 @('10000, 0, 0, 15000', '10000,0,0,5000'),
                 @('1000000, 0, 0, 950000', '1000000,300000,0,250000'),
                 @('100000, 0, 0, 95000', '100000,5000,0,0'),
                 @('33333.33, 11111.11, 2222.22', '33333.33,12000,34666.66,0'),
                 @('0, 5000, 0, 5000', '0,750,0,750'),
                 @('0, 0', '0,0,0,0'))) {
    Same "SmallBusinessPool: ($($r[0]))" "LET(x, $sb($($r[0])), $rows)" $r[1]
}
Near 'SmallBusinessPool: the s 328-210 example, Cassidy, deducts 15,700' "INDEX($sb(28500, 16800, 0, 29600), 2, 1)" '15700'
# Boundary cases at 0 and $20,000, one per line: name|arguments|row (2 deduction, 3 closing,
# 4 assessable)|expected|tolerance, then name|arguments|all 4 rows to 2 places.
$sbEdges = @'
cents summing to exactly 20,000 are not below it, deduction|2420135.56, 2116636.38, 0, 4516771.94|2|1043536.125|0.000001
cents summing to exactly 20,000 are not below it, assessable|2420135.56, 2116636.38, 0, 4516771.94|4|1023536.125|0.000001
2E-8 under 20,000 on 240,000 added is deducted in full|0, 130000, 0, 110000.00000002|2|19999.99999998|0.000001
2E-8 under 20,000 on 240,000 added closes at 0|0, 130000, 0, 110000.00000002|3|0|0.000001
2E-7 below 0 on 2,200,000 added deducts nothing|1100000, 0, 0, 1100000.0000002|2|0|0.000001
2E-7 below 0 on 2,200,000 added is assessable|1100000, 0, 0, 1100000.0000002|4|0.0000002|1E-9
1E-15 on 2 trillion added is deducted in full|1E12, 1E-15, 0, 1E12|2|1E-15|1E-17
1E-15 on 2 trillion added is not assessable|1E12, 1E-15, 0, 1E12|4|0|1E-17
15 decimal places under 10 cents are kept|0, 0.012345678901234|2|0.012345678901234|1E-18
1E-15 under 20,000 is deducted in full|19999, 0.999999999999999|2|19999.999999999999|0.000001
1E-15 under 20,000 closes at 0|19999, 0.999999999999999|3|0|0.000001
3E-15 over 0 is deducted in full|0.123456789012344, 0, 0, 0.123456789012341|2|3E-15|1E-17
3E-15 over 0 is not assessable|0.123456789012344, 0, 0, 0.123456789012341|4|0|1E-17
3E-15 below 0 deducts nothing|0.123456789012341, 0, 0, 0.123456789012344|2|0|1E-17
3E-15 below 0 is assessable|0.123456789012341, 0, 0, 0.123456789012344|4|3E-15|1E-17
a 15-digit pool under 1 keeps its 15th digit|0.123456789012341, 0|2|0.123456789012341|1E-17
a trillionth under 20,000 is deducted in full|19000, 999.999999999999|2|19999.999999999999|0.000001
a trillionth under 20,000 closes at 0|19000, 999.999999999999|3|0|0.000001
a cent under 20,000 on 6 trillion added is deducted in full|3E12, 19999.99, 0, 3E12|2|19999.99|0.000001
a cent under 20,000 on 6 trillion added closes at 0|3E12, 19999.99, 0, 3E12|3|0|0.000001
a cent under 20,000 on 6 trillion added is not assessable|3E12, 19999.99, 0, 3E12|4|0|0.000001
20,000 on 6 trillion added deducts 30% and 15%|3E12, 20000, 0, 3E12|2|900000003000|0.01
20,000 on 6 trillion added is assessable above the pool|3E12, 20000, 0, 3E12|4|899999983000|0.01
a cent over 20,000 on 6 trillion added deducts 30% and 15%|3E12, 20000.01, 0, 3E12|2|900000003000.0015|0.01
a cent below 0 on 6 trillion added deducts nothing|3E12, 0, 0, 3000000000000.01|2|0|0.000001
a cent below 0 on 6 trillion added is assessable|3E12, 0, 0, 3000000000000.01|4|0.01|0.000001
0 on 6 trillion added deducts 30%|3E12, 0, 0, 3E12|2|900000000000|0.01
a cent over 0 on 6 trillion added is deducted in full|3E12, 0.01, 0, 3E12|2|0.01|0.000001
a cent over 0 on 6 trillion added is not assessable|3E12, 0.01, 0, 3E12|4|0|0.000001
a fraction of a cent under 20,000 is deducted in full|20000, 0, 0, 0.0000002|2|19999.9999998|0.000001
a fraction of a cent under 20,000 closes at 0|20000, 0, 0, 0.0000002|3|0|0.000001
a fraction of a cent below 0 deducts nothing|0.1, 0.2, 0, 0.3000001|2|0|0.000001
a fraction of a cent below 0 is assessable|0.1, 0.2, 0, 0.3000001|4|0.0000001|1E-12
cents summing to exactly 0 are not above it, deduction|0.1, 0.2, 0, 0.3|2|0.06|0.000001
cents summing to exactly 0 are not above it, assessable|0.1, 0.2, 0, 0.3|4|0.06|0.000001
0.1 x 0.1, binary noise on the 1E-15 grid, is accepted|0, 0.1*0.1|2|0.01|1E-18
'@
foreach ($f in $sbEdges -split "`n" | ForEach-Object { ,($_.Trim() -split '\|') }) {
    Near "SmallBusinessPool: $($f[0])" "INDEX($sb($($f[1])), $($f[2]), 1)" $f[3] $f[4]
}
$sbEdgeRows = @'
a cent under 20,000 (15000.55, 5000.44, 0, 1)|15000.55, 5000.44, 0, 1|15000.55,19999.99,0,0
20,000 (15000.55, 5000.45, 0, 1)|15000.55, 5000.45, 0, 1|15000.55,5250.23,14749.77,0
a cent over 20,000 (15000.55, 5000.46, 0, 1)|15000.55, 5000.46, 0, 1|15000.55,5250.23,14749.78,0
a cent below 0 (5000.4, 0, 0, 5000.41)|5000.4, 0, 0, 5000.41|5000.4,0,0,0.01
0 (5000.4, 0, 0, 5000.4)|5000.4, 0, 0, 5000.4|5000.4,1500.12,0,1500.12
a cent over 0 (5000.4, 0.01, 0, 5000.4)|5000.4, 0.01, 0, 5000.4|5000.4,0.01,0,0
'@
foreach ($f in $sbEdgeRows -split "`n" | ForEach-Object { ,($_.Trim() -split '\|') }) {
    Same "SmallBusinessPool: $($f[0])" "LET(x, $sb($($f[1])), $rows)" $f[2]
}
Same 'SmallBusinessPool: two years' "LET(x, $sb(100000, {28000,0}, 0, {8000,0}), $rows)" '100000,85800,34200,25740,85800,60060,0,0'
Same 'SmallBusinessPool: a low pool value in a later year' "LET(x, $sb(30000, {0,0,0}), $rows)" '30000,21000,14700,9000,6300,14700,21000,14700,0,0,0,0'
Same 'SmallBusinessPool: a blank opening is 0' "LET(x, $sb(Z1, 100), $rows)" '0,100,0,0'
Near 'SmallBusinessPool: an error opening shows' "--ISNA(INDEX($sb(NA(), 100), 1, 1))" '1'
Near 'SmallBusinessPool: an error amount shows' "--ISNA(INDEX($sb(100000, NA()), 2, 1))" '1'
$sbShape = 'OpeningBalance must be one value, Additions one row with a value for each income year, and CostAdditions and TerminationValues one value or one for each year.'
$sbValue = "SmallBusinessPool$L needs the opening balance and each amount to be a number from 0 to under 10,000,000,000,000, with no digit past the 15th decimal place, or blank."
foreach ($r in @(@('{1,2}, 100', $sbShape), @('100, {1;2}', $sbShape), @('100, {1,2}, {1,2,3}', $sbShape),
                 @('-1, 100', $sbValue), @('100, -1', $sbValue), @('100, 1, , "5"', $sbValue), @('100, TRUE', $sbValue),
                 @('1E12, 1E-16, 0, 1E12', $sbValue), @('0, 0.0123456789012345', $sbValue), @('1E-16, 0', $sbValue))) {
    Same "SmallBusinessPool refuses ($($r[0]))" "LET(x, $sb($($r[0])), $one)" $r[1]
}

# --- Movement. Each row's opening is its own: a blank one defaults only that row.
Near 'Movement, per-row opening'       "INDEX($mv({100;`"`";50},{110,120;5,6;60,70}),1,1)" '10'
Near 'Movement, blank opening row'     "INDEX($mv({100;`"`";50},{110,120;5,6;60,70}),2,1)" '5'
Near 'Movement, default opening'       "INDEX($mv(,{100,110,130,100}),1,4)" '-30'

# --- Corkscrew. SUM leaves text and TRUE out of a total, so a flow typed as text showed
# in the flows row but never reached the closing balance, and nothing checked Opening.
# On the scratch sheet AE1:AF1 holds 1 and 2, and AI1:AL1 holds 3 blanks then TBA.
$ck = "oz.Corkscrew$L"
$ckOpening = "Corkscrew$L needs Opening to be a single number."
$ckFlow    = "Corkscrew$L needs every flow to be a number or blank."
foreach ($r in @(@('100, {1,"2"}', $ckFlow), @('100, {1,2}, {TRUE,0}', $ckFlow), @('100, AI1:AL1', $ckFlow),
                 @('AE1:AF1, {1,2}', $ckOpening), @('"100", {1,2}', $ckOpening), @('TRUE, {1,2}', $ckOpening))) {
    Same "Corkscrew refuses ($($r[0]))" "LET(x, $ck($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}
Near 'Corkscrew: a blank Opening is nil'     "SUM(ABS($ck(Z1, {1,2}) - $ck(0, {1,2})))" '0'
Near 'Corkscrew: an empty-text flow is nil'  "SUM(ABS(TAKE($ck(100, {1,`"`"}), -1) - {101,101}))" '0'
Near 'Corkscrew: an empty-text Opening is nil' "SUM(ABS($ck(`"`", {1,2}) - $ck(0, {1,2})))" '0'
Near 'Corkscrew: blank flow cells are nil'     "SUM(ABS(TAKE($ck(100, AI1:AK1), -1) - {100,100,100}))" '0'
Near 'Corkscrew: range flows match an array' "SUM(ABS($ck(100, AE1:AH1) - $ck(100, {1,2,3,4})))" '0'
Near 'Corkscrew: an error in a flow shows'   "--ISNA(INDEX($ck(100, {1,#N/A}), 3, 2))" '1'
Near 'Corkscrew: an error in Opening shows'  "--ISNA(INDEX($ck(NA(), {1,2}), 3, 2))" '1'

# --- CorkScrewReversal. A flag of 1 reverses the period's opening balance, 0 or a blank
# leaves it. The flag multiplied the balance, so a 2 reversed it twice over and the
# period closed on minus its opening; TRUE and the text "1" passed as 1.
$cr = "oz.CorkScrewReversal$L"
Same 'CorkScrewReversal, a 1 reverses its period' "TEXTJOIN(`",`",FALSE,TAKE($cr(100,{0,1,0},{10,20,30}),-1))" '110,20,50'
Near 'CorkScrewReversal, the reversal row'        "INDEX($cr(100,{0,1,0},{10,20,30}),2,2)" '-110'
Same 'CorkScrewReversal, blank and 1 flag cells'  "TEXTJOIN(`",`",FALSE,TAKE($cr(100,AD1:AE1,{10,20}),-1))" '110,20'
# A refusal is the message alone in one cell, so returning the help or a schedule fails.
$crRefused = 'ReversalFlags must be 1, 0 or blank in every period.'
foreach ($flag in '2', '-1', '0.5', 'TRUE', '"1"') {
    Same "CorkScrewReversal refuses a flag of $flag" `
         "LET(x, $cr(100,{0,$flag,0},{10,20,30}), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $crRefused
}
Same 'CorkScrewReversal refuses a text flag cell' "LET(x, $cr(100,AI1:AL1,{10,20,30,40}), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $crRefused
# INDEX read past a narrower flags row, so it returned #REF! instead of a message,
# dropped the extra columns of a wider one and read only the first row of a deeper one.
# All are refused; AD1:AE1 is 2 cells wide and AD1:AE2 is 2 rows deep.
$crWidth = 'ReversalFlags needs one row with one flag for each column of Flow1.'
foreach ($r in '100, {0,1}, {10,20,30}', '100, 0, {10,20,30}', '100, {0,1,0,1}, {10,20,30}', '100, AD1:AE1, {10,20,30}',
               '100, {0,1;1,0}, {10,20}', '100, AD1:AE2, {10,20}') {
    Same "CorkScrewReversal refuses flags of another shape ($r)" "LET(x, $cr($r), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $crWidth
}
# It totalled its flows with SUM and checked neither Opening nor flows, so a flow typed as
# text showed but never reached the closing balance. It now takes Corkscrew's checks.
$crvOpening = "CorkScrewReversal$L needs Opening to be a single number."
$crvFlow    = "CorkScrewReversal$L needs every flow to be a number or blank."
foreach ($r in @(@('100, {0,1}, {1,"2"}', $crvFlow), @('100, {0,1}, {1,2}, {TRUE,0}', $crvFlow),
                 @('100, {0,0,0,1}, AI1:AL1', $crvFlow), @('AE1:AF1, {0,1}, {1,2}', $crvOpening),
                 @('"100", {0,1}, {1,2}', $crvOpening), @('TRUE, {0,1}, {1,2}', $crvOpening))) {
    Same "CorkScrewReversal refuses ($($r[0]))" "LET(x, $cr($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}
Near 'CorkScrewReversal: an empty-text Opening is nil' "SUM(ABS($cr(`"`", {0,1}, {1,2}) - $cr(0, {0,1}, {1,2})))" '0'
# AD1 is blank and AE1:AF1 holds 1 and 2: numbers and blank cells give the same result as before.
Near 'CorkScrewReversal: numbers and blank cells' "SUM(ABS($cr(100, AD1:AE1, {10,20}, AE1:AF1) - {100,111;0,-111;10,20;1,2;111,22}))" '0'
# Z1 and AI1:AJ1 are blank: a blank Opening and blank flow cells are nil, as in Corkscrew.
Near 'CorkScrewReversal: blank Opening and flow cells are nil' "SUM(ABS($cr(Z1, {0,1}, AI1:AJ1) - $cr(0, {0,1}, {0,0})))" '0'

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
Same 'FY blank start month'    "$fy(DATE(2026,8,15),Z1)" 'FY2027'
Same 'FY empty start month'    "$fy(DATE(2026,8,15),`"`")" 'FY2027'
Same 'FY zero start unchanged' "$fy(DATE(2026,8,15),0)" 'FY2026'
Same 'FY per-row start month'  "TEXTJOIN(`"|`",FALSE,$fy(DATE(2026,8,15),{`"`";1;7}))" 'FY2027|FY2026|FY2027'
Same 'FY blank month boundary' "TEXTJOIN(`"|`",FALSE,$fy(DATE(2026,6,30)+{0;1},Z1))" 'FY2026|FY2027'
Same 'FY help with no args'    "INDEX($fy(),1,1)" 'FUNCTION:'

# Duplicate effective dates use the last matching rate, including item schedules.
Near 'ScheduleRates: last duplicate wins' "INDEX(oz.ScheduleRates${L}({2,3},{1;1},{10;20}),1,1)" '20'
Near 'ScheduleRatesByItems: last duplicate wins' "INDEX(oz.ScheduleRatesByItems${L}({2,3},`"item`",{`"item`";`"item`"},{1;1},{10;20}),1,1)" '20'

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

# A mismatched input must produce a diagnostic, never silently discard periods or rows.
foreach ($fn in $dsf, $dsv, $lrv) {
    $cover = if ($fn -eq $dsf) { '1.2' } else { '{1.2,1.2}' }
    $apr = if ($fn -eq $dsf) { '0.06' } else { '{0.06,0.06}' }
    foreach ($debt in '{1000,0;999999,999999}', '{1000;0}') {
        Same "Debt rejects non-row debt: $fn $debt" "IF(ISTEXT(INDEX($fn(,$debt,{300,300},$cover,$apr,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
    }
    foreach ($cash in '{300,300,999999}', '300', '{300,300;999999,999999}') {
        Same "Debt rejects mismatched cash: $fn $cash" "IF(ISTEXT(INDEX($fn(,{1000,0},$cash,$cover,$apr,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
    }
    $cover3 = if ($fn -eq $dsf) { '1.2' } else { '{1.2,1.2,1.2}' }
    $apr3 = if ($fn -eq $dsf) { '0.06' } else { '{0.06,0.06,0.06}' }
    Same "Debt rejects a shorter cash row: $fn" "IF(ISTEXT(INDEX($fn(,{1000,0,0},{300,300},$cover3,$apr3,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
    if ($fn -eq $dsf) {
        Same 'Fixed debt rejects a DSCR column' "IF(ISTEXT(INDEX($fn(,{1000,0},{300,300},{1.2;1.2},0.06,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
        Same 'Fixed debt rejects an APR column' "IF(ISTEXT(INDEX($fn(,{1000,0},{300,300},1.2,{0.06;0.06},12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
    }
    if ($fn -ne $dsf) {
        Same "Debt rejects a shorter DSCR row: $fn" "IF(ISTEXT(INDEX($fn(,{1000,0,0},{300,300,300},$cover,$apr3,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
        Same "Debt rejects a shorter APR row: $fn" "IF(ISTEXT(INDEX($fn(,{1000,0,0},{300,300,300},$cover3,$apr,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
        Same "Debt rejects a shorter period-rate row: $fn" "IF(ISTEXT(INDEX($fn(,{1000,0,0},{300,300,300},$cover3,,, {0.01,0.01}),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
        foreach ($row in '{1.2,1.2,1.2}', '1.2', '{1.2,1.2;1.2,1.2}') {
            Same "Debt rejects mismatched DSCR: $fn $row" "IF(ISTEXT(INDEX($fn(,{1000,0},{300,300},$row,$apr,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
            Same "Debt rejects mismatched APR: $fn $row" "IF(ISTEXT(INDEX($fn(,{1000,0},{300,300},$cover,$row,12),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
            Same "Debt rejects mismatched period rates: $fn $row" "IF(ISTEXT(INDEX($fn(,{1000,0},{300,300},$cover,,, $row),1,1)),`"diagnostic`",`"number`")" 'diagnostic'
        }
    }
}

# 1,000 drawn in period 1, 300 of cash a period, 1.2 times covered, 6% a year, 5 years.
# Rows are opening balance, interest, MINUS the principal repayment, closing balance.
$sched = "$lrv(, {1000,0,0,0,0}, {300,300,300,300,300}, {1.2,1.2,1.2,1.2,1.2}, {0.06,0.06,0.06,0.06,0.06}, 12)"

Near 'Debt: repayments retire the principal exactly' "SUM(INDEX($sched, 3, 0))" '-1000' '0.0000001'
Near 'Debt: schedule ends at zero'                   "INDEX($sched, 4, 5)"      '0'     '0.0000001'
Near 'Debt: closing = opening less repayment' `
     "SUMPRODUCT(ABS(INDEX($sched,4,0) - INDEX($sched,1,0) - INDEX($sched,3,0)))" '0' '0.0000001'
# No new debt after period 1, so every opening must be the previous closing. Line the 2
# rows up by dropping the first opening and the last closing rather than by position.
Near 'Debt: each opening = the last closing' `
     "SUMPRODUCT(ABS(DROP(INDEX($sched,1,0),,1) - DROP(INDEX($sched,4,0),,-1)))" '0' '0.0000001'
Near 'Debt: cash used never exceeds CFADS/DSCR' `
     "MAX(INDEX($sched,2,0) - INDEX($sched,3,0)) - 250" '0' '0.0000001'
Near 'Debt: balance never goes negative'             "MIN(0, MIN(INDEX($sched, 4, 0)))" '0' '0.0000001'

# Cash well over the debt must clear it in one period, not leave twice the interest behind.
Near 'Debt: surplus cash clears the balance' `
     "INDEX($lrv(, {1000,0}, {1800,1800}, {1.2,1.2}, {0.06,0.06}, 12), 4, 1)" '0' '0.0000001'
# No cash must capitalise one period of interest, not 2.
Near 'Debt: no cash capitalises interest once' `
     "LET(s, $lrv(, {1000,0}, {0,0}, {1.2,1.2}, {0.06,0.06}, 12), INDEX(s,4,1) - 1000 - INDEX(s,2,1))" '0' '0.0000001'

# A cash deficit supplies no debt service. Unpaid interest still capitalises.
Near 'Debt LRV: a deficit cannot create debt from nil' `
     "SUM(ABS($lrv(, {0,0}, {-300,-50}, {1.2,1.2}, {0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt LRV: deficits capitalise only unpaid interest' `
     "MAX(ABS($lrv(, {1000,0}, {-300,-50}, {1.2,1.2}, {0.06,0.06}, 12)-$lrv(, {1000,0}, {0,0}, {1.2,1.2}, {0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt LRV: a deficit cannot reopen repaid debt' `
     "SUM(ABS(DROP($lrv(, {1000,0,0}, {1800,-300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12),,1)))" '0' '0.0000001'
# Uncapped balances solve closing = opening + rate*(opening+closing)/2 - MAX(cash,0)/DSCR.
# When cash clears the balance, principal repayment is capped at opening debt.
$lrvMixedExpected = "VSTACK(HSTACK(1000,103000/97,101075/97,5154825/4753),HSTACK(6000/97,7775/97,202150/4753,1030965/38024),HSTACK(6000/97,-11625/97,202150/4753,-5154825/4753),HSTACK(103000/97,91375/97,5154825/4753,0))"
Near 'Debt LRV: mixed cash uses each period and carries its closing' `
     "MAX(ABS($lrv(500,{500,0,100,0},{-300,300,-50,1800},{1.2,1.5,1.1,1.2},{0.06,0.08,0.04,0.05},12)-$lrvMixedExpected))" '0' '0.0000001'
Near 'Debt LRV: explicit rates preserve mixed-cash balances' `
     "MAX(ABS($lrv(500,{500,0,100,0},{-300,300,-50,1800},{1.2,1.5,1.1,1.2},,,{0.06,0.08,0.04,0.05})-$lrvMixedExpected))" '0' '0.0000001'
Near 'Debt LRV: a zero-rate deficit cannot increase debt' `
     "MAX(ABS($lrv(,{1000,0},{-300,300},{1.2,1.2},{0,0},12)-{1000,1000;0,0;0,-250;1000,750}))" '0' '0.0000001'

# Required cash inputs must be numeric, including each cell in a supplied range.
foreach ($rawCash in 'Z1:AA1', 'Y1:Z1', '{"300","0"}', '{FALSE,TRUE}', '{0,""}', '{0,#N/A}') {
    Same "Debt LRV: rejects non-numeric cash $rawCash" `
         "INDEX($lrv(,{1000,0},$rawCash,{1.2,1.2},{0.06,0.06},12),1,1)" `
         'CFADS must be one row of numbers with the same number of periods as Debt'
}

# The other 2 sculpting functions pay the whole debt service, so their balances differ,
# but the same roll-forward has to hold: closing = opening + interest - debt service.
foreach ($fn in $dsf, $dsv) {
    $arg = if ($fn -eq $dsf) { '1.2, 0.06' } else { '{1.2,1.2,1.2}, {0.06,0.06,0.06}' }
    $s2 = "$fn(, {1000,0,0}, {300,300,300}, $arg, 12)"
    Near "Debt: roll-forward holds for $fn" `
         "SUMPRODUCT(ABS(INDEX($s2,4,0) - INDEX($s2,1,0) - INDEX($s2,2,0) - INDEX($s2,3,0)))" '0' '0.0000001'

    # Negative CFADS leaves no cash for debt service, so the period must repay nothing and
    # roll the balance forward with one period of interest. Until this change the payment
    # went negative, which lifted the closing balance above opening plus interest and
    # printed row 3, the negated repayment, as a positive receipt.
    $arg2 = if ($fn -eq $dsf) { '1.2, 0.06' } else { '{1.2,1.2}, {0.06,0.06}' }
    $s3 = "$fn(, {1000,0}, {-300,-300}, $arg2, 12)"
    Near "Debt: negative CFADS repays nothing in $fn"            "INDEX($s3,3,1)" '0' '0.0000001'
    Near "Debt: negative CFADS capitalises interest once in $fn" `
         "INDEX($s3,4,1) - INDEX($s3,1,1) - INDEX($s3,2,1)" '0' '0.0000001'

    # A negative amount owed repays nothing in the returned rows exactly as in the SCAN,
    # so period 1's closing balance is period 2's opening. Before the rows were floored
    # too, they paid the negative amount and closed at zero while the SCAN carried it.
    $s4 = "$fn(-100, {0,0}, {300,300}, $arg2, 12)"
    Near "Debt: a negative amount owed repays nothing in $fn"        "INDEX($s4,3,1)" '0' '0.0000001'
    Near "Debt: a negative balance rolls into the next period in $fn" `
         "INDEX($s4,4,1) - INDEX($s4,1,2)" '0' '0.0000001'
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

# --- TimelineOffset. The interval is read off the timeline's first 2 dates and, up to
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
# floored by that count. 200 dates at 3-day steps, starting 3 months before the
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

# The 2 worked examples printed in the function's own help, which a reader is meant to
# copy. Neither could be run as printed: the call was missing its 2 closing brackets.
Near 'TimelineOffset: documented example, inside the timeline' `
     "$to(`"15/2/2026`", EDATE(`"1/1/2026`", SEQUENCE( , 12, 0)))" '1'
Near 'TimelineOffset: documented example, before the timeline' `
     "$to(`"15/2/2025`", EDATE(`"1/1/2026`", SEQUENCE( , 12, 0)))" '-11'


# --- Amortise on a timeline shorter than a month. The period length is read off the first
# 2 dates and rounded to whole months, which is nought below about a fortnight, and the
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
# --- Default timeline edges. A month-end start lands its last EDATE in a shorter month, which
# DATEDIF "M" under-counted, cropping the final repayment; and a one-month loan has a one-date
# timeline with no second date to measure. Row 6 is the principal portion of each payment.
$amEnd = "$am(1400, 0, 14, DATE(2026,1,31))"
$amEndRate = "$am(1400, 0.12, 14, DATE(2026,1,31))"
$amOne = "$am(100, 0, 1, DATE(2026,1,1))"
Near 'Amortise: month-end start keeps all 14 months on the default timeline' "COLUMNS($amEnd)" '14'
Near 'Amortise: month-end start repays the whole principal'         "ABS(SUM(INDEX($amEnd,6,0)))" '1400' '0.0000001'
Near 'Amortise: month-end start at 12% keeps all 14 months'         "COLUMNS($amEndRate)" '14'
Near 'Amortise: month-end start at 12% repays the whole principal'  "ABS(SUM(INDEX($amEndRate,6,0)))" '1400' '0.0000001'
Near 'Amortise: one-month loan is a schedule, not an error'         "SUMPRODUCT(--ISERROR($amOne))" '0'
Near 'Amortise: one-month loan repays the whole principal'          "ABS(SUM(INDEX($amOne,6,0)))" '100' '0.0000001'
# --- Depreciation edges: a one-year life has no preceding years to schedule, and a
# fractional straight-line life keeps its remainder instead of dropping it.
Near 'DB: one-year life depreciates cost less salvage'   "SUM(oz.DB$L(1000, 100, 1))" '900'
Near 'DDB: one-year life depreciates cost less salvage'  "SUM(oz.DDB$L(1000, 100, 1))" '900'
Near 'SLN: 1.5-year life depreciates the whole cost'     "SUM(oz.SLN$L(120, 0, 1.5))" '120'
Near 'SLN: 1.5-year life takes 2 columns'                "COLUMNS(oz.SLN$L(120, 0, 1.5))" '2'
Near 'SLN: half-year life depreciates the whole cost'    "SUM(oz.SLN$L(120, 0, 0.5))" '120'
# Both timelines open on 1 January 2026, so month one is period one on each. Month 2 opens
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
# A 20-day period is no whole number of months either, and used to round to one and be
# laid out as though it were a month long.
Near 'Amortise: twenty-day periods hold the same money as monthly' `
     "SUM($am(10000, 0.05, 12, DATE(2026,1,1), DATE(2026,1,1) + SEQUENCE( , 20, 0) * 20)) - SUM($amMo)" '0' '0.0000001'
# An unevenly spaced sub-monthly timeline must still tile the calendar: no month counted
# twice, none dropped. This one alternates 5 and 20 day periods.
$uneven = "DATE(2026,1,1) + SCAN(0, SEQUENCE( , 27, 0), LAMBDA(a,k, IF(k = 0, 0, a + IF(MOD(k,2) = 1, 5, 20))))"
Near 'Amortise: an uneven timeline still counts each month once' `
     "SUM($am(10000, 0.05, 12, DATE(2026,1,1), $uneven)) - SUM($amMo)" '0' '0.0000001'
# These aligned monthly, quarterly and six-monthly totals match v2.5.0.
$amL = "10000, 0.05, 48, DATE(2026,1,1)"
Near 'Amortise: monthly unchanged'    "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 24, 0))))"     '377875.31' '0.005'
Near 'Amortise: quarterly unchanged'  "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 3)))" '132616.32' '0.005'
Near 'Amortise: six-monthly unchanged' "SUM($am($amL, EDATE(DATE(2026,1,1), SEQUENCE( , 8, 0) * 6)))" '92617.78' '0.005'
# --- A supplied timeline has to advance. The window the schedule is cropped to ends at
# MAX(Timeline) plus the gap before it, so a repeated or out-of-order date moved that end
# and admitted a different number of months: 1 January to 1 June with 1 March repeated
# returned 5,772.61 of principal where the same span without the repeat returns 4,937.63.
$amSpan = "EDATE(DATE(2026,1,1), {0,1,2,3,4,5})"
$amRepeat = "EDATE(DATE(2026,1,1), {0,1,2,2,3,4,5})"
$amBackwards = "DATE(2026,1,1) + {0,60,30}"
Near 'Amortise: a timeline that advances is unchanged' `
     "SUM(INDEX($am(10000, 0.05, 12, DATE(2026,1,1), $amSpan),6,0))" '4937.63' '0.005'
Same 'Amortise: a repeated timeline date is refused' `
     "LEFT(INDEX($am(10000, 0.05, 12, DATE(2026,1,1), $amRepeat),1,1),8)" 'Timeline'
Same 'Amortise: an out-of-order timeline date is refused' `
     "LEFT(INDEX($am(10000, 0.05, 12, DATE(2026,1,1), $amBackwards),1,1),8)" 'Timeline'
# --- FinalPeriodEnd. The last period has no successor to measure, so it runs as long as
# the one before it, and a sub-monthly timeline that stops before the loan does drops every
# month past that inferred end. Stating the end keeps them. Ten weekly periods from
# 1 January 2026 end on 5 March and run to 12 March, so January, February and March are the
# only months inside them.
$amWkShort = "DATE(2026,1,1) + SEQUENCE( , 10, 0) * 7"
$amStop = "$am(10000, 0.05, 12, DATE(2026,1,1), $amWkShort)"
$amCarry = "$am(10000, 0.05, 12, DATE(2026,1,1), $amWkShort, DATE(2027,1,1))"
Near 'Amortise: a short weekly timeline stops at the end it infers' `
     "SUMPRODUCT(--(INDEX($amStop,4,0)<>0)) - 3" '0'
Near 'Amortise: a stated final period end carries every payment' `
     "SUM(INDEX($amCarry,4,0)) - SUM(INDEX($amMo,4,0))" '0' '0.0000001'
Near 'Amortise: a stated final period end carries every month of interest' `
     "SUM(INDEX($amCarry,3,0)) - SUM(INDEX($amMo,3,0))" '0' '0.0000001'
# Handed the end it would have inferred, the parameter reproduces the default exactly.
Near 'Amortise: the stated end matches the inferred one' `
     "SUM($am(10000, 0.05, 12, DATE(2026,1,1), $amWkShort, MAX($amWkShort) + 7)) - SUM($amStop)" '0' '0.0000001'
# A period that holds several months adds their flows, but rows 2 and 5 are balances: it
# opens on its first month's balance and closes on its last month's. The last of these 10
# weekly periods holds April to December, and adding its balances opened it at 37,942.09
# and closed a loan repaid in December at 30,395.51. The row sums above cannot see that.
Near 'Amortise: a period holding several months opens on its first month' `
     "INDEX($amCarry,2,10) - INDEX($amMo,2,4)" '0' '0.0000001'
Near 'Amortise: a period holding several months closes on its last month' `
     "INDEX($amCarry,5,10) - INDEX($amMo,5,12)" '0' '0.0000001'
Near 'Amortise: a period holding one month keeps its balance' `
     "INDEX($amCarry,5,9) - INDEX($amMo,5,3)" '0' '0.0000001'
# A longer period after shorter ones holds several months too: 4 weekly periods, then
# quarters from 1 February, so the fifth period holds February to April.
$amMixed = "$am(10000, 0.05, 12, DATE(2026,1,1), HSTACK(DATE(2026,1,1) + SEQUENCE( , 4, 0) * 7, EDATE(DATE(2026,2,1), SEQUENCE( , 3, 0) * 3)))"
Near 'Amortise: a quarter after weekly periods opens on its first month' `
     "INDEX($amMixed,2,5) - INDEX($amMo,2,2)" '0' '0.0000001'
Near 'Amortise: a quarter after weekly periods closes on its last month' `
     "INDEX($amMixed,5,5) - INDEX($amMo,5,4)" '0' '0.0000001'
Same 'Amortise: a final period end inside the timeline is refused' `
     "LEFT(INDEX($am(10000, 0.05, 12, DATE(2026,1,1), $amWkShort, DATE(2026,1,15)),1,1),14)" 'FinalPeriodEnd'
# The default timeline is monthly, where the boundary moves nothing, so the end is refused
# without a supplied timeline rather than measured against one this function chose.
Same 'Amortise: a final period end without a timeline is refused' `
     "LEFT(INDEX($am(10000, 0.05, 12, DATE(2026,1,1), , DATE(2027,1,1)),1,1),14)" 'FinalPeriodEnd'
Same 'Amortise: help with no args' "INDEX($am(),1,1)" 'FUNCTION:'

# Monthly cash flows follow the supplied period dates. Expected vectors use a
# 60-digit recurrence and explicit month counts; controls retain prior results.
Near 'Amortise dates: annual_offset_zero' `
     "MAX(ABS(oz.Amortise${L}(2400,0,24,DATE(2027,1,1),EDATE(DATE(2026,7,1),{0,12,24}))-{2400,0,0;2400,1800,600;0,0,0;-600,-1200,-600;1800,600,0;600,1200,600}))" '0' '0.0000001'
Near 'Amortise dates: annual_offset_interest' `
     "MAX(ABS(oz.Amortise${L}(100000,0.06,24,DATE(2027,1,1),EDATE(DATE(2026,7,1),{0,12,24}))-{100000,0,0;100000,76110.755853565599,26133.13551617966;2703.1220052197464,3207.1119659223432,459.23063547448169;-26592.366151654143,-53184.732303308287,-26592.366151654143;76110.755853565599,26133.13551617966,-1.22e-54;23889.244146434397,49977.620337385939,26133.13551617966}))" '0' '0.0000001'
Near 'Amortise dates: quarter_offset_zero' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,2,1),EDATE(DATE(2027,1,1),{0,3,6}))-{600,0,0;600,400,100;0,0,0;-200,-300,-100;400,100,0;200,300,100}))" '0' '0.0000001'
Near 'Amortise dates: uneven_long' `
     "MAX(ABS(oz.Amortise${L}(1200,0,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,1,4,6}))-{1200,0,0,0;1200,1100,800,600;0,0,0,0;-100,-300,-200,-200;1100,800,600,400;100,300,200,200}))" '0' '0.0000001'
Near 'Amortise dates: annual_exact_boundary' `
     "MAX(ABS(oz.Amortise${L}(2400,0,24,DATE(2027,7,1),EDATE(DATE(2026,7,1),{0,12,24}))-{0,2400,0;0,2400,1200;0,0,0;0,-1200,-1200;0,1200,0;0,1200,1200}))" '0' '0.0000001'
Near 'Amortise dates: annual_short_loan' `
     "MAX(ABS(oz.Amortise${L}(100,0,1,DATE(2027,1,1),EDATE(DATE(2026,7,1),{0,12,24}))-{100,0,0;100,0,0;0,0,0;-100,0,0;0,0,0;100,0,0}))" '0' '0.0000001'
Near 'Amortise dates: quarter_before_window' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2026,12,1),EDATE(DATE(2027,1,1),{0,3,6}))-{0,0,0;500,200,0;0,0,0;-300,-200,0;200,0,0;300,200,0}))" '0' '0.0000001'
Near 'Amortise dates: annual_leap_boundary' `
     "MAX(ABS(oz.Amortise${L}(2400,0,24,DATE(2027,3,1),EDATE(DATE(2027,1,1),{0,12,24}))-{2400,0,0;2400,1400,200;0,0,0;-1000,-1200,-200;1400,200,0;1000,1200,200}))" '0' '0.0000001'
Near 'Amortise dates: monthly_offset_zero' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,15),EDATE(DATE(2027,1,1),{0,1,2,3,4,5}))-{600,0,0,0,0,0;600,500,400,300,200,100;0,0,0,0,0,0;-100,-100,-100,-100,-100,-100;500,400,300,200,100,0;100,100,100,100,100,100}))" '0' '0.0000001'
Near 'Amortise dates: monthly_aligned' `
     "MAX(ABS(oz.Amortise${L}(1200,0.12,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,1,2,3,4,5}))-{1200,0,0,0,0,0;1200,1105.3814535859899,1009.8167217078399,913.29634251090897,815.81075952200695,717.35032070321813;12,11.0538145358599,10.098167217078398,9.1329634251090894,8.1581075952200699,7.1735032070321818;-106.61854641401001,-106.61854641401001,-106.61854641401001,-106.61854641401001,-106.61854641401001,-106.61854641401001;1105.3814535859899,1009.8167217078397,913.29634251090829,815.81075952200808,717.35032070321699,617.90527749624027;94.618546414010012,95.564731878150113,96.520379196931614,97.485582988900916,98.460438818789939,99.445043206977829}))" '0' '0.0000001'
Near 'Amortise dates: quarter_aligned' `
     "MAX(ABS(oz.Amortise${L}(1200,0.12,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,3,6,9}))-{1200,0,0,0;1200,913.29634251090897,617.90527749623834,313.56356782055946;33.151981752938298,24.464574227361339,15.513929566351846,6.292071421469088;-319.85563924203007,-319.85563924203007,-319.85563924203007,-319.85563924203007;913.29634251090829,617.90527749624027,313.56356782056014,-1.5347723092418164e-12;286.70365748909177,295.3910650146687,304.3417096756782,313.56356782056099}))" '0' '0.0000001'
Near 'Amortise dates: sixmonth_aligned' `
     "MAX(ABS(oz.Amortise${L}(1200,0.12,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,6}))-{1200,0;1200,617.90527749623834;57.616555980299637,21.806000987820934;-639.71127848406002,-639.71127848406002;617.90527749623959,-7.9580786405131221e-13;582.09472250376041,617.90527749623914}))" '0' '0.0000001'
Near 'Amortise dates: monthend_truncated' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,31),EDATE(DATE(2027,1,31),{0,1,2}))-{600,0,0;600,500,400;0,0,0;-100,-100,-100;500,400,300;100,100,100}))" '0' '0.0000001'
Near 'Amortise dates: monthend_february' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,31),EDATE(DATE(2027,1,31),{0,1}))-{600,0;600,500;0,0;-100,-100;500,400;100,100}))" '0' '0.0000001'
Near 'Amortise dates: one_date_timeline' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,1),DATE(2027,1,1))-{600;600;0;-100;500;100}))" '0' '0.0000001'
Near 'Amortise dates: two_loans' `
     "MAX(ABS(CHOOSEROWS(oz.Amortise${L}({600;1800},{0;0},{6;9},VSTACK(DATE(2027,2,1),DATE(2027,3,1)),EDATE(DATE(2027,1,1),{0,3,6,9})),{1,2,3,4,5,6,8,9,10,11,12,13})-{600,0,0,0;600,400,100,0;0,0,0,0;-200,-300,-100,0;400,100,0,0;200,300,100,0;1800,0,0,0;1800,1600,1000,400;0,0,0,0;-200,-600,-600,-400;1600,1000,400,0;200,600,600,400}))" '0' '0.0000001'
Near 'Amortise dates: multiple loans keep their blank separator' `
     "SUM(--(CHOOSEROWS($am({600;1800},{0;0},{6;9},VSTACK(DATE(2027,2,1),DATE(2027,3,1)),EDATE(DATE(2027,1,1),{0,3,6,9})),7)="" ""))" '4'
Near 'Amortise dates: monthly final end retains its existing effect' `
     "MAX(ABS($am(1200,0,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,1,2}),DATE(2027,7,1))-$am(1200,0,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,1,2}))))" '0' '0.0000001'
Near 'Amortise dates: quarterly final end retains its existing effect' `
     "MAX(ABS($am(1200,0,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,3}),DATE(2028,1,1))-$am(1200,0,12,DATE(2027,1,1),EDATE(DATE(2027,1,1),{0,3}))))" '0' '0.0000001'

# February clipping must retain the timeline calendar anchor.
Near 'Amortise dates: one_february_start' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,2,28),DATE(2027,2,28))-{600;600;0;-100;500;100}))" '0' '0.0000001'
Near 'Amortise dates: day28_truncation' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,28),EDATE(DATE(2027,1,28),{0,1}))-{600,0;600,500;0,0;-100,-100;500,400;100,100}))" '0' '0.0000001'
Near 'Amortise dates: day30_truncation' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,1,30),EDATE(DATE(2027,1,30),{0,1}))-{600,0;600,500;0,0;-100,-100;500,400;100,100}))" '0' '0.0000001'
Near 'Amortise dates: day30_late_start' `
     "MAX(ABS(oz.Amortise${L}(600,0,6,DATE(2027,3,29),EDATE(DATE(2027,1,30),{0,1}))-{0,600;0,600;0,0;0,-100;0,500;0,100}))" '0' '0.0000001'

Near 'Amortise dates: default multiple-loan timeline' `
     "MAX(ABS(CHOOSEROWS(oz.Amortise${L}({600;1800},{0;0},{6;9},VSTACK(DATE(2027,2,1),DATE(2027,3,1))),{1,2,3,4,5,6,8,9,10,11,12,13})-{600,0,0,0,0,0,0,0,0,0;600,500,400,300,200,100,0,0,0,0;0,0,0,0,0,0,0,0,0,0;-100,-100,-100,-100,-100,-100,0,0,0,0;500,400,300,200,100,0,0,0,0,0;100,100,100,100,100,100,0,0,0,0;0,1800,0,0,0,0,0,0,0,0;0,1800,1600,1400,1200,1000,800,600,400,200;0,0,0,0,0,0,0,0,0,0;0,-200,-200,-200,-200,-200,-200,-200,-200,-200;0,1600,1400,1200,1000,800,600,400,200,0;0,200,200,200,200,200,200,200,200,200}))" '0' '0.0000001'

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
# The last period on a monthly timeline ended EDATE(its own start, 1) - 1, and EDATE steps
# from 28 February to 28 March, not 31 March, so a month-end timeline closed 3 days early
# and an asset in service in those days landed in no period at all. EOMONTH is the step
# DayCountRate already takes. Row 1 is the cost, which appears in the period holding
# the in-service date and nowhere else.
$dpME = "EOMONTH(DATE(2025,12,31), SEQUENCE( , 3, 0))"
Near 'Depreciate: a month-end timeline runs its last period to the month end' `
     "SUMPRODUCT(--(INDEX($dp(10000, DATE(2026,3,29), 5, $dpME),1,0)<>0)) - 1" '0'
Near 'Depreciate: a month-end timeline still holds the cost once' `
     "SUM(INDEX($dp(10000, DATE(2026,3,29), 5, $dpME),1,0)) - 10000" '0' '0.005'
# Whole-month intervals are unchanged. Two, 4 and 6 months never failed: the SWITCH
# lookups that only listed 1, 3 and 12 were read by 2 bindings nothing else read, so
# Excel never evaluated them. They are gone rather than generalised.
# Summing the whole block cannot see the depreciation row: BookValue is OpeningAmount less
# Depreciation on the same period, so the 2 cancel cell for cell and the total collapses to
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
# A life in years is a life, not a date. Transposing arguments 2 and 3 puts 46,023
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
# Depreciate is not guarded the same way, on purpose. It reads its period length off the
# first pair and crops to EDATE(MAX(Timeline), months per period), neither of which a later
# repeat moves, so a repeated date leaves the money where it was. Pinned here so the
# difference from Amortise is a checked result rather than an assumption.
Near 'Depreciate: a repeated timeline date collects the same money' `
     "SUM(INDEX($dp($dpA, $amRepeat),3,0)) - SUM(INDEX($dp($dpA, $amSpan),3,0))" '0' '0.005'
Same 'Depreciate: help with no args' "INDEX($dp(),1,1)" 'FUNCTION:'
# Revenue from Sale (row 5) is the salvage value in the disposal period, not a sale
# price, as the help says; DV and PC set salvage to 0, so their row is 0.
$dpTL24 = "EDATE(DATE(2026,1,1), SEQUENCE( , 24, 0))"
Near 'Depreciate: revenue from sale is the salvage value' `
     "SUM(INDEX($dp(10000, DATE(2026,1,1), 1, $dpTL24, 1000),5,0))" '1000'
Near 'Depreciate: DV revenue from sale is 0' `
     "SUM(INDEX($dp(10000, DATE(2026,1,1), 1, $dpTL24, 1000, , , `"DV`"),5,0))" '0'
Near 'Depreciate: PC revenue from sale is 0' `
     "SUM(INDEX($dp(10000, DATE(2026,1,1), 1, $dpTL24, 1000, , , `"PC`"),5,0))" '0'

# --- Diagnostic companions. The workbook guide sends a reader to these when a schedule
# looks wrong, so they must accept every input the parent calculates. Amortise and
# Depreciate both take a sub-monthly timeline and Amortise takes a zero APR, and all 3
# were reported as problems: the reader was told to change data that was already right.
$amDV = "oz.Amortise${L}DV"
$dpDV = "oz.Depreciate${L}DV"
$wkTL = "DATE(2026,1,1) + SEQUENCE( , 52, 0) * 7"
$moTL = "EDATE(DATE(2026,1,1), SEQUENCE( , 14, 0))"
$flatTL = "DATE(2026,1,1) + {0,0,0}"
$dupTL = "DATE(2026,1,1) + {0,30,30}"
$okTL = "DATE(2026,1,1) + {0,30,60}"
Near 'Amortise DV: a weekly timeline is accepted'   "--($amDV(10000, 0.05, 12, DATE(2026,1,1), $wkTL))" '1'
Near 'Amortise DV: a zero APR is accepted'          "--($amDV(10000, 0, 12, DATE(2026,1,1), $moTL))" '1'
Near 'Amortise DV: a monthly timeline is accepted'  "--($amDV(10000, 0.05, 12, DATE(2026,1,1), $moTL))" '1'
Same 'Amortise DV: a negative APR is still refused' `
     "LEFT(INDEX($amDV(10000, -0.05, 12, DATE(2026,1,1), $moTL),1,1),3)" 'APR'
Same 'Amortise DV: a timeline that does not advance is still refused' `
     "LEFT(INDEX($amDV(10000, 0.05, 12, DATE(2026,1,1), $flatTL),1,1),8)" 'Timeline'
# The companion has to refuse every FinalPeriodEnd the parent refuses, and accept the one
# it calculates. An error in it reaches the parent through MAX, so it is an error in an
# argument like any other rather than a value the shape test can read.
Near 'Amortise DV: a final period end after the timeline is accepted' `
     "--($amDV(10000, 0.05, 12, DATE(2026,1,1), $wkTL, MAX($wkTL) + 30))" '1'
Same 'Amortise DV: a final period end inside the timeline is refused' `
     "LEFT(INDEX($amDV(10000, 0.05, 12, DATE(2026,1,1), $wkTL, DATE(2026,1,15)),1,1),14)" 'FinalPeriodEnd'
Same 'Amortise DV: a final period end without a timeline is refused' `
     "LEFT(INDEX($amDV(10000, 0.05, 12, DATE(2026,1,1), , DATE(2027,1,1)),1,1),14)" 'FinalPeriodEnd'
Same 'Amortise DV: an error in the final period end is refused' `
     "LEFT(INDEX($amDV(10000, 0.05, 12, DATE(2026,1,1), $wkTL, NA()),1,1),14)" 'FinalPeriodEnd'
Near 'Depreciate DV: a weekly timeline is accepted'  "--($dpDV(10000, DATE(2026,1,1), 5, $wkTL))" '1'
Near 'Depreciate DV: a monthly timeline is accepted' "--($dpDV(10000, DATE(2026,1,1), 5, $moTL))" '1'
Same 'Depreciate DV: a timeline that does not advance is still refused' `
     "LEFT(INDEX($dpDV(10000, DATE(2026,1,1), 5, $flatTL),1,1),11)" 'Each period'
# Depreciate refuses a fractional life outright, so the companion has to say so too.
Same 'Depreciate DV: a fractional life is refused' `
     "LEFT(INDEX($dpDV(10000, DATE(2026,1,1), 1.5, $moTL),1,1),11)" 'LifeInYears'
Near 'Depreciate DV: a one-year life is accepted'  "--($dpDV(10000, DATE(2026,1,1), 1, $moTL))" '1'
# Only the first gap was measured, so a date repeated later in the row passed the
# companion while the parent's approximate MATCH buckets both copies into one period.
Same 'Amortise DV: a repeated date later in the timeline is refused' `
     "LEFT(INDEX($amDV(10000, 0.05, 12, DATE(2026,1,1), $dupTL),1,1),8)" 'Timeline'
Same 'Depreciate DV: a repeated date later in the timeline is refused' `
     "LEFT(INDEX($dpDV(10000, DATE(2026,1,1), 5, $dupTL),1,1),11)" 'Each period'
Near 'Amortise DV: a three-date timeline that advances is accepted' `
     "--($amDV(10000, 0.05, 12, DATE(2026,1,1), $okTL))" '1'
Near 'Depreciate DV: a three-date timeline that advances is accepted' `
     "--($dpDV(10000, DATE(2026,1,1), 5, $okTL))" '1'
# Corkscrew's companion accepts number flows, arrays and ranges alike, and names the first
# argument holding text or an error. AL1 holds TBA.
$ckDV = "oz.Corkscrew${L}DV"
Near 'Corkscrew DV: number flows and a range are accepted' "--($ckDV(100, {1,2}, AE1:AH1))" '1'
Same 'Corkscrew DV: a text Opening is refused'   "LEFT(INDEX($ckDV(AL1, {1,2}),1,1),11)" 'Set Opening'
Same 'Corkscrew DV: a text flow is refused'      "LEFT(INDEX($ckDV(100, {1,2}, AL1),1,1),9)" 'Set Flow2'
Same 'Corkscrew DV: an error in a flow is named' "LEFT(INDEX($ckDV(100, {1,#N/A}),1,1),5)" 'Flow1'
# It refused what Corkscrew accepts as nil: a blank or empty-text Opening and blank or
# empty-text flow cells (the Corkscrew checks above), and gave an error for a two-cell Opening,
# which Corkscrew refuses. Z1 and AI1:AK1 are blank.
foreach ($r in @('Z1, {1,2}', '"", {1,2}', '100, AI1:AK1', '100, {1,""}')) {
    Near "Corkscrew DV: accepted as Corkscrew accepts it ($r)" "--($ckDV($r))" '1'
}
Same 'Corkscrew DV: a two-cell Opening is refused' "LEFT(INDEX($ckDV(AE1:AF1, {1,2}),1,1),11)" 'Set Opening'

# --- Character counts. Both count by splitting the text on the characters asked for, so
# empty text had nothing to split and a comma could not be one of the characters: each
# returned #VALUE! for an input the help permits. A single character is now taken as
# itself rather than as a comma separated list.
$cc = "oz.CountC$L"
$ccu = "oz.CountCU$L"
Near 'CountC: empty text counts nought'   "$cc(`"`", `"a`")" '0'
Near 'CountCU: empty text counts nought'  "$ccu(`"`", `"a`")" '0'
Near 'CountC: a comma is a character'     "$cc(`"a,b,c`", `",`")" '2'
Near 'CountCU: a comma is a character'    "$ccu(`"a,b,c`", `",`")" '2'
Near 'CountC: a CSV still counts each character'  "$cc(`"aAbba`", `"a,b`")" '4'
Near 'CountCU: a CSV still counts each character' "$ccu(`"aAbba`", `"a,b`")" '4'
Near 'CountC: the count is case sensitive'  "$cc(`"aAbba`", `"A`")" '1'
Near 'CountCU: the count is case sensitive' "$ccu(`"aAbba`", `"A`")" '1'

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

# --- The last 2 help-text misspellings in the library.
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
# Carrying liability includes the payment due now. Remeasure before settling it.
# The vectors use exact fractions from discounting the remaining payment at 5%.
Near 'Remeasure advance: unchanged payments leave the asset unchanged' `
     "MAX(ABS($lrem({100,100},0.05,100+100/1.05,200,TRUE)-VSTACK(2000/21,0,200,0)))" '0' '0.0000001'
Near 'Remeasure advance: increased payments raise the asset' `
     "MAX(ABS($lrem({110,110},0.05,100+100/1.05,200,TRUE)-VSTACK(2200/21,410/21,4610/21,0)))" '0' '0.0000001'
Near 'Remeasure advance: decreased payments reduce the asset' `
     "MAX(ABS($lrem({90,90},0.05,100+100/1.05,200,TRUE)-VSTACK(600/7,-410/21,3790/21,0)))" '0' '0.0000001'
Near 'Remeasure advance: only the excess reduction reaches profit or loss' `
     "MAX(ABS($lrem({10,10},0.05,100+100/1.05,5,TRUE)-VSTACK(200/21,-1230/7,0,-1195/7)))" '0' '0.0000001'
Near 'Remeasure advance: a column keeps the same payment timing' `
     "MAX(ABS($lrem({110;110},0.05,100+100/1.05,200,TRUE)-VSTACK(2200/21,410/21,4610/21,0)))" '0' '0.0000001'
foreach ($payment in '110', '{110}') {
    Near "Remeasure advance: a sole payment $payment leaves no unpaid liability" `
         "MAX(ABS($lrem($payment,0.05,100,200,TRUE)-VSTACK(0,10,210,0)))" '0' '0.0000001'
}
Near 'Remeasure advance: zero-rate adjustment excludes settlement' `
     "MAX(ABS($lrem({110,110},0,200,200,TRUE)-VSTACK(110,20,220,0)))" '0' '0.0000001'
foreach ($timing in 'FALSE', 'Z1', '""') {
    Near "Remeasure arrears: timing $timing keeps the ordinary result" `
         "LET(pv,NPV(0.05,110,110),MAX(ABS($lrem({110,110},0.05,185.94,181.55,$timing)-VSTACK(pv,pv-185.94,181.55+pv-185.94,0))))" '0' '0.0000001'
}
Same 'Remeasure: help with no args' "INDEX($lrem(),1,1)" 'FUNCTION:'

# --- Return on equity. Net income over AVERAGE shareholders' equity, which is what the
# help and the page it links have always said; until 6 Sep 2026 the function divided by
# whatever it was handed. The 2-argument call must return exactly what it did, because
# the ratios worksheet caches it.
$roe = "oz.ROE$L"
Near 'ROE: two arguments, as cached'   "$roe(59.972, AVERAGE(251.635, 256.144))" '0.236213' '0.0000005'
Near 'ROE: opening equity averages in' "$roe(59.972, 256.144, 251.635)"          '0.236213' '0.0000005'
Near 'ROE: blank opening cell = closing' "$roe(59.972, 253.8895, Z1)"            '0.236213' '0.0000005'
Near 'ROE: rows of periods'            "SUM($roe({10,20}, {100,200}, {100,200}))" '0.2' '0.0000001'
Near 'Sheet: ratios ROE unchanged'     "'oz.FinancialRatios'!A47"                 '0.236213' '0.0000005'

# Retention uses current-period earnings left after dividends, divided by net income.
Near 'Sheet: retention is one less the payout ratio' "'oz.FinancialRatios'!A61-(1-22.112/41.981)" '0' '0.000000000001'
Same 'Sheet: retention formula display' "'oz.FinancialRatios'!B61" "=oz.RetentionRatio$L( 19.869, 41.981)"

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
# A column of PeriodsPerYear is read row by row: one blank defaults its own row to 12.
# OR() once gave every row 12 as soon as one cell was blank.
Near 'PeriodRate: per-row periods'              "INDEX($pr({0.05;0.05},{1;`"`"}),1,1)" '0.05' '0.0000000001'
Near 'PeriodRate: blank periods row = 12'       "INDEX($pr({0.05;0.05},{1;`"`"}),2,1)" '0.00407412' '0.000000005'
Near 'AnnualRate: per-row periods'              "INDEX($ar({0.01;0.01},{4;`"`"}),1,1)" '0.04060401' '0.000000005'
Near 'AnnualRate: blank periods row = 12'       "INDEX($ar({0.01;0.01},{4;`"`"}),2,1)" '0.12682503' '0.000000005'

# --- Day count conventions. July 2026 has 31 days, September 30, and the year 365.
$dc = "oz.DayCountRate$L"
Near 'DayCount: three-year periods include every leap year' "SUMPRODUCT(ABS($dc(DATE({2026,2029,2032},1,1),0.1,4)-0.3))" '0' '0.0000000001'
Near 'DayCount: multi-year end dates' "SUMPRODUCT(ABS($dc(DATE({2028,2031,2034},12,31),0.1,4,TRUE)-0.3))" '0' '0.0000000001'
Near 'DayCount: partial first year then two whole years' "INDEX($dc(EDATE(DATE(2027,7,1),{0,30,60}),0.1,4),1,1)" '0.1*(184/365+2)' '0.0000000001'
Near 'DayCount: multi-year column dates and rates' "SUMPRODUCT(ABS($dc(DATE({2026;2029;2032},1,1),{0.1;0.2;0.3},4)-{0.3,0.6,0.9}))" '0' '0.0000000001'
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
# Actual/Actual on a week that straddles 31 December: 4 days in one year, 3 in the
# next, each over its own year's length. Common to leap, then leap to common.
Near 'DayCount: Actual/Actual, common into leap' "INDEX($dc(DATE(2023,12,28) + {0,7,14}, 0.073, 4),1,1) - 0.073*(4/365 + 3/366)" '0' '0.0000000001'
Near 'DayCount: Actual/Actual, leap into common' "INDEX($dc(DATE(2024,12,28) + {0,7,14}, 0.073, 4),1,1) - 0.073*(4/366 + 3/365)" '0' '0.0000000001'
Near 'DayCount: Actual/Actual, financial year'  "INDEX($dc(EDATE(DATE(2023,7,1), {0,12,24}), 0.073, 4),1,1) - 0.073*(184/365 + 182/366)" '0' '0.0000000001'
Near 'DayCount: one date is an error'         "--ISERROR($dc(DATE(2026,7,1), 0.073))" '1'
Near 'DayCount: unknown convention is an error' "--ISERROR($dc($jul, 0.073, 5))" '1'

# --- Level payment. Checked against PMT() where the rate is flat, and under actual day
# counts by running the loan forward period by period, payment off at each period end,
# which is not the discount-factor sum the function solves.
$lp = "oz.LevelPayment$L"
$lpR = "$dc(EDATE(DATE(2026,7,1), SEQUENCE(1, 24, 0)), 0.073, 4)"
Near 'LevelPayment: agrees with PMT, 30 years monthly' "$lp(250000, SEQUENCE(1, 360, 0.005, 0)) + PMT(0.005, 360, 250000)" '0' '0.0000001'
Near 'LevelPayment: agrees with PMT with a balloon'    "$lp(100000, SEQUENCE(1, 60, 0.004, 0), 20000) + PMT(0.004, 60, 100000, -20000)" '0' '0.0000001'
Near 'LevelPayment: actual days close to nil' `
     "LET(r, $lpR, x, $lp(50000, r), REDUCE(50000, r, LAMBDA(b, k, b * (1 + k) - x)))" '0' '0.0000001'
Near 'LevelPayment: actual days close on the balloon' `
     "LET(r, $lpR, x, $lp(50000, r, 10000), REDUCE(50000, r, LAMBDA(b, k, b * (1 + k) - x)))" '10000' '0.0000001'
Near 'LevelPayment: nil rates divide the principal'      "$lp(1200, {0,0,0})"      '400'
Near 'LevelPayment: a blank balloon cell is none'        "$lp(1000, {0.01,0.01,0.01}, Z1) - $lp(1000, {0.01,0.01,0.01})" '0'
# A refusal is its message alone in one cell, so the help, a number or the wrong message fails.
$lpAmount = "LevelPayment$L needs Principal and Balloon to be single numbers."
$lpRates  = "LevelPayment$L needs Rates to be one row of numbers above -1."
foreach ($r in @(@('1000, {0.01,-1}', $lpRates), @('1000, {0.01;0.01}', $lpRates), @('"1000", {0.01,0.01}', $lpAmount),
                 @('1000, {0.01,0.01}, TRUE', $lpAmount), @('1000, AI1:AK1', $lpRates), @('1000, Z1', $lpRates),
                 @('{1000,2000}, {0.01,0.01}', $lpAmount))) {
    Same "LevelPayment refuses ($($r[0]))" "LET(x, $lp($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}
Near 'LevelPayment: an error in Rates shows' "--ISNA($lp(1000, {0.01,#N/A}))" '1'

# --- Hire purchase and chattel mortgage schedules, their journals, lease journals and the
# finance comparison. The schedules are checked against cases worked by hand in cents, against
# PMT(), and by identity: every column rolls forward, the last closing balance is the
# balloon and the principal repaid is the amount financed less the balloon. Every journal
# must balance in each period and leave its liability at nil.
$hps = "oz.HPSchedule$L"
$hpj = "oz.HPJournal$L"
$lj  = "oz.LeaseJournal$L"
$fc  = "oz.FinanceCompare$L"
$pr  = "oz.PeriodRate$L"
# The largest gap between a period's debits and credits, over every period of journal $j.
function Unbalanced($j) {
    "LET(j, $j, b, DROP(j, 1), p, CHOOSECOLS(b, 1), d, IF(ISNUMBER(CHOOSECOLS(b, 3)), CHOOSECOLS(b, 3), 0)," +
    " c, IF(ISNUMBER(CHOOSECOLS(b, 4)), CHOOSECOLS(b, 4), 0), MAX(MAP(UNIQUE(p), LAMBDA(k, ABS(SUM((p = k) * (d - c)))))))"
}
# Debits less credits on account $a over the whole of journal $j.
function Ledger($j, $a) {
    "LET(j, $j, b, DROP(j, 1), d, IF(ISNUMBER(CHOOSECOLS(b, 3)), CHOOSECOLS(b, 3), 0)," +
    " c, IF(ISNUMBER(CHOOSECOLS(b, 4)), CHOOSECOLS(b, 4), 0), SUM((CHOOSECOLS(b, 2) = `"$a`") * (d - c)))"
}

# Worked by hand: 1,000 at 10% a year, 3 yearly payments. The level payment is 402.1148,
# 402.11 in cents; interest is 100.00, 69.789 and 36.557, so the last payment is
# 365.57 + 36.56 = 402.13 and takes the 2 cents of rounding.
Same 'HPSchedule: hand-worked, 3 yearly payments in arrears' `
     "TEXTJOIN(`",`",FALSE,TEXT($hps(1000,0.1,3,1),`"0.00`"))" `
     '1000.00,697.89,365.57,402.11,402.11,402.13,100.00,69.79,36.56,302.11,332.32,365.57,697.89,365.57,0.00'
# In advance with a 100 balloon: the payment is (1,000 - 100 / 1.331) / 2.486852 / 1.1 =
# 338.09; interest runs on 661.91 and 390.01 after each payment; the last is 100 * 0.1 / 1.1
# = 9.09, so 90.91 left after the last payment grows to the balloon.
Same 'HPSchedule: hand-worked, in advance with a balloon' `
     "TEXTJOIN(`",`",FALSE,TEXT($hps(1000,0.1,3,1,TRUE,100),`"0.00`"))" `
     '1000.00,728.10,429.01,338.09,338.09,338.10,66.19,39.00,9.09,271.90,299.09,329.01,728.10,429.01,100.00'
foreach ($rate in '0', '0.0721', '0.15') {
    foreach ($n in '1', '12', '60') {
        foreach ($adv in 'FALSE', 'TRUE') {
            foreach ($bal in '0', '10000') {
                $s = "$hps(50000,$rate,$n,12,$adv,$bal)"
                $tag = "rate $rate, $n payments, advance $adv, balloon $bal"
                Near "HPSchedule closes on the balloon, $tag" "INDEX($s,5,$n)" $bal
                Near "HPSchedule shape, $tag" "ROWS($s)*1000+COLUMNS($s)" "5000+$n"
                Near "HPSchedule rolls forward each period, $tag" `
                     "SUMPRODUCT(ABS(CHOOSEROWS($s,1)+CHOOSEROWS($s,3)-CHOOSEROWS($s,2)-CHOOSEROWS($s,5)))" '0' '0.0000001'
                Near "HPSchedule principal repays the amount financed less the balloon, $tag" `
                     "SUM(CHOOSEROWS($s,4))" "50000-$bal" '0.0000001'
                Near "HPSchedule pays, charges and owes whole cents, $tag" `
                     "LET(x, CHOOSEROWS($s,2,3,5), SUMPRODUCT(--(ABS(x*100-ROUND(x*100,0))>0.000001)))" '0'
            }
        }
    }
}
# A schedule of several payments: each opens on the last closing balance, and every payment
# but the last is PMT()'s in cents.
foreach ($rate in '0', '0.0721', '0.15') {
    foreach ($n in '12', '60') {
        foreach ($adv in 'FALSE', 'TRUE') {
            foreach ($bal in '0', '10000') {
                $s = "$hps(50000,$rate,$n,12,$adv,$bal)"
                $tag = "rate $rate, $n payments, advance $adv, balloon $bal"
                Near "HPSchedule each opening is the closing before it, $tag" `
                     "SUMPRODUCT(ABS(DROP(CHOOSEROWS($s,1),,1)-DROP(CHOOSEROWS($s,5),,-1)))" '0'
                Near "HPSchedule payment is PMT() in cents, $tag" `
                     "INDEX($s,2,1)" "ROUND(-PMT($pr($rate,12),$n,50000,-$bal,--$adv),2)"
                Near "HPSchedule only the last payment differs, $tag" `
                     "SUMPRODUCT(--(DROP(CHOOSEROWS($s,2),,-1)<>INDEX($s,2,1)))" '0'
            }
        }
    }
}
# Discounting the payments and the balloon at the agreement's own rate gives back the
# amount financed, less the interest rounding: half a cent a period at most.
foreach ($adv in 'FALSE', 'TRUE') {
    $s = "$hps(50000,0.0721,60,12,$adv,10000)"
    $flows = if ($adv -eq 'TRUE') { "HSTACK(CHOOSEROWS($s,2),10000)" } else { "HSTACK(0,CHOOSEROWS($s,2)+IF(SEQUENCE(1,60)=60,10000,0))" }
    Near "HPSchedule present value is the amount financed, advance $adv" "INDEX($fc($flows,0.0721,12),2,3)" '50000' '0.3'
}
Near 'HPSchedule weekly closes to nil' "INDEX($hps(10000,0.08,52,52),5,52)" '0'
Near 'HPSchedule weekly rate is the annual rate over 52' `
     "INDEX($hps(10000,0.08,52,52),3,1)" "ROUND(10000*$pr(0.08,52),2)"
Near 'HPSchedule blank optional cells are the defaults' "SUM(ABS($hps(1000,0.1,3,Z1,Z2,Z3)-$hps(1000,0.1,3)))" '0'
$hpTerms = "HPSchedule$L needs single numbers: Financed above 0 and a Balloon from 0 in whole cents, AnnualRate above -1, a whole number of Periods from 1 and PeriodsPerYear above 0, and TRUE, FALSE, 1 or 0 for InAdvance."
$hpBalloon = "HPSchedule$L needs a Balloon the payments can reach: no more than Financed grown at the rate."
foreach ($r in @(@('0, 0.1, 3', $hpTerms), @('1000, -1, 3', $hpTerms), @('1000, 0.1, 0', $hpTerms),
                 @('1000, 0.1, 2.5', $hpTerms), @('1000, 0.1, 3, 0', $hpTerms), @('1000, 0.1, 3, 12, "yes"', $hpTerms),
                 @('1000, 0.1, 3, 12, "0"', $hpTerms), @('1000, 0.1, 3, 12, "1"', $hpTerms),
                 @('1000, 0.1, 3, 12, {0,1}', $hpTerms), @('1000, 0.1, 3, 12, {TRUE;FALSE}', $hpTerms),
                 @('1000, 0.1, 3, 12, FALSE, -1', $hpTerms), @('"1000", 0.1, 3', $hpTerms), @('{1000,2000}, 0.1, 3', $hpTerms),
                 @('1000, "10%", 3', $hpTerms), @('1000, 0.1, 3, 1, FALSE, 2000', $hpBalloon),
                 @('1000, 0.1, 3, 1, FALSE, 1331.01', $hpBalloon), @('1000, 0.1, 3, 1, TRUE, 1331.01', $hpBalloon),
                 @('1000.005, 0.1, 3', $hpTerms), @('1000, 0.1, 3, 1, FALSE, 100.005', $hpTerms), @('1000, 0.1, 3, 12, 2', $hpTerms),
                 @('257446.20001, 0.1, 3, 1', $hpTerms), @('257446.20 + 5E-10, 0.1, 3, 1', $hpTerms),
                 @('1000, 0.1, 3, 1, FALSE, 100.000001', $hpTerms))) {
    Same "HPSchedule refuses ($($r[0]))" "LET(x, $hps($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}
# 1,000 grown at 10% for 3 years is 1,331: that Balloon is reached with nil payments, and a
# cent more is refused above rather than scheduled with a final payment of -0.01.
# Amounts in whole cents may carry binary noise: 10.1 * 3 is 30.299999999999997.
Near 'HPSchedule takes arithmetic noise as whole cents' "INDEX($hps(10.1*3,0.1,3,1),1,1)" '30.3'
Near 'HPSchedule accepts binary noise on a six-figure amount' "INDEX($hps(257446.20 + 5E-11,0.1,3,1),1,1)" '257446.20'
Near 'HPSchedule takes 1 for InAdvance as TRUE' "SUM(ABS($hps(1000,0.1,3,1,1)-$hps(1000,0.1,3,1,TRUE)))" '0'
# In advance the last interest is the Balloon's share Rate / (1 + Rate), which can be a cent off
# interest on the balance left after the last payment, so the help must say so.
Near 'HPSchedule help states the last interest in advance' "--ISNUMBER(SEARCH(`"Rate / (1 + Rate)`", TEXTJOIN(`" `", TRUE, $hps())))" '1'
foreach ($adv in 'FALSE', 'TRUE') {
    Near "HPSchedule reaches a Balloon of Financed grown at the rate, advance $adv" `
         "SUMPRODUCT(ABS(CHOOSEROWS($hps(1000,0.1,3,1,$adv,1331),2)))" '0'
}

# --- Hire purchase journal
Same 'HPJournal: hand-worked, one payment and a balloon' `
     "TEXTJOIN(`"|`",FALSE,$hpj($hps(1000,0.1,1,1,,500)))" `
     ('Period|Account|Debit|Credit|Narration|0|Asset at cost|1000||Amount financed|0|Hire purchase liability||1000|Amount financed|' +
      '1|Interest expense|100||Interest on the agreement|1|Hire purchase liability||100|Interest on the agreement|' +
      '1|Hire purchase liability|600||Instalment|1|Cash||600|Instalment|1|Hire purchase liability|500||Balloon|1|Cash||500|Balloon')
foreach ($adv in 'FALSE', 'TRUE') {
    foreach ($bal in '0', '10000') {
        $s = "$hps(50000,0.0721,36,12,$adv,$bal)"
        $j = "$hpj($s)"
        $tag = "advance $adv, balloon $bal"
        Near "HPJournal balances in every period, $tag" (Unbalanced $j) '0' '0.0000001'
        Near "HPJournal leaves the liability at nil, $tag" (Ledger $j 'Hire purchase liability') '0' '0.0000001'
        Near "HPJournal debits the asset with the amount financed, $tag" (Ledger $j 'Asset at cost') '50000'
        Near "HPJournal charges the schedule's interest, $tag" (Ledger $j 'Interest expense') "SUM(CHOOSEROWS($s,3))"
        Near "HPJournal pays the instalments and the balloon, $tag" (Ledger $j 'Cash') "-SUM(CHOOSEROWS($s,2))-$bal"
        Near "HPJournal runs from period 0 to 36, $tag" "MAX(DROP(TAKE($j,,1),1))*100+MIN(DROP(TAKE($j,,1),1))" '3600'
    }
}
# A negative amount swaps its debit and credit: interest at a negative rate reduces the liability.
Same 'HPJournal swaps a negative interest, debit' "INDEX($hpj($hps(1000,-0.05,2,1)),4,2)" 'Hire purchase liability'
Same 'HPJournal swaps a negative interest, credit' "INDEX($hpj($hps(1000,-0.05,2,1)),5,2)" 'Interest expense'
Same 'HPJournal renames the liability for a chattel mortgage' `
     "INDEX($hpj($hps(1000,0.1,3,1),{`"Motor vehicle`",`"Chattel mortgage loan`"}),3,2)" 'Chattel mortgage loan'
Same 'HPJournal keeps the default for a blank account' `
     "INDEX($hpj($hps(1000,0.1,3,1),{`"`",`"Chattel mortgage loan`"}),2,2)" 'Asset at cost'
Near 'HPJournal takes an account code' "INDEX($hpj($hps(1000,0.1,3,1),{1200}),2,2)" '1200'
$hpjSchedule = "HPJournal$L needs the five rows of HPSchedule$L(), all numbers."
foreach ($r in @(@('{1;2;3}', $hpjSchedule), @("$hps(0,0.1,3)", $hpjSchedule),
                 @("$hps(1000,0.1,3,1), SEQUENCE(1,5)", "HPJournal$L takes at most four account names."))) {
    Same "HPJournal refuses ($($r[0]))" "LET(x, $hpj($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}

# --- Lease journal, from LeaseSchedule and ROUSchedule
Same 'LeaseJournal: hand-worked, one payment in arrears' `
     "TEXTJOIN(`"|`",FALSE,$lj($lsch({105},0.05),$lrou(100,1)))" `
     ('Period|Account|Debit|Credit|Narration|0|Right-of-use asset|100||Lease liability recognised at commencement|' +
      '0|Lease liability||100|Lease liability recognised at commencement|1|Interest expense|5||Interest on the lease liability|' +
      '1|Lease liability||5|Interest on the lease liability|1|Lease liability|105||Lease payment|1|Cash||105|Lease payment|' +
      '1|Depreciation expense|100||Depreciation of the right-of-use asset|1|Accumulated depreciation||100|Depreciation of the right-of-use asset')
foreach ($rate in '0', '0.004') {
    foreach ($n in '1', '3', '12') {
        foreach ($adv in 'FALSE', 'TRUE') {
            $p = "SEQUENCE(,$n,100,0)"
            $cost = "($lli($p,$rate,$adv)+IF($adv,100,0))"
            $ls = "$lsch($p,$rate,$adv)"
            $j = "$lj($ls,$lrou($cost,$n))"
            $tag = "rate $rate, $n periods, advance $adv"
            Near "LeaseJournal balances in every period, $tag" (Unbalanced $j) '0' '0.0000001'
            Near "LeaseJournal leaves the lease liability at nil, $tag" (Ledger $j 'Lease liability') '0' '0.0000001'
            Near "LeaseJournal debits the asset with its cost, $tag" (Ledger $j 'Right-of-use asset') $cost
            Near "LeaseJournal depreciates the whole cost, $tag" (Ledger $j 'Accumulated depreciation') "-$cost"
            Near "LeaseJournal pays every payment, $tag" (Ledger $j 'Cash') "-100*$n"
            Near "LeaseJournal charges the schedule's interest, $tag" (Ledger $j 'Interest expense') "SUM(CHOOSEROWS($ls,3))"
            Near "LeaseJournal runs to the last period, $tag" "MAX(DROP(TAKE($j,,1),1))" $n
        }
    }
}
# A payment at commencement is the rest of the asset's cost, taken from cash at period 0.
Near 'LeaseJournal takes a commencement payment from cash at period 0' `
     "LET(j, $lj($lsch({100,100},0.05,TRUE),$lrou(100+$lli({100,100},0.05,TRUE),2)), SUM((TAKE(j,,1)=0)*(INDEX(j,,2)=`"Cash`")*IF(ISNUMBER(INDEX(j,,4)),INDEX(j,,4),0)))" '100'
# A negative amount swaps its debit and credit: interest at a negative rate reduces the liability.
# Payments in advance, period by period: the first payment from cash at commencement, then
# interest and a payment in periods 1 and 2, and depreciation alone in the last period.
Same 'LeaseJournal: in advance, every line in its period' `
     "TEXTJOIN(`";`",FALSE,BYROW(DROP($lj($lsch({100,100,100},0.05,TRUE),$lrou(100+$lli({100,100,100},0.05,TRUE),3)),1),LAMBDA(r,INDEX(r,1)&`",`"&INDEX(r,2)&`",`"&TEXT(N(INDEX(r,3))-N(INDEX(r,4)),`"0.00`"))))" `
     ('0,Right-of-use asset,185.94;0,Lease liability,-185.94;0,Right-of-use asset,100.00;0,Cash,-100.00;' +
      '1,Interest expense,9.30;1,Lease liability,-9.30;1,Lease liability,100.00;1,Cash,-100.00;' +
      '1,Depreciation expense,95.31;1,Accumulated depreciation,-95.31;' +
      '2,Interest expense,4.76;2,Lease liability,-4.76;2,Lease liability,100.00;2,Cash,-100.00;' +
      '2,Depreciation expense,95.31;2,Accumulated depreciation,-95.31;' +
      '3,Depreciation expense,95.31;3,Accumulated depreciation,-95.31')
# An incentive larger than the other commencement costs comes back from cash; the narration
# must fit either direction.
Same 'LeaseJournal narrates the rest of the cost either way' "INDEX($lj($lsch({105},0.05),$lrou(90,1)),4,5)" "Rest of the right-of-use asset's cost at commencement"
Same 'LeaseJournal swaps a negative interest' "INDEX($lj($lsch({100},-0.01),$lrou($lli({100},-0.01),1)),4,2)" 'Lease liability'
Same 'LeaseJournal renames an account' `
     "INDEX($lj($lsch({105},0.05),$lrou(100,1),{`"ROU - vehicle`"}),2,2)" 'ROU - vehicle'
Same 'LeaseJournal keeps the default for a blank account' `
     "INDEX($lj($lsch({105},0.05),$lrou(100,1),{`"`",`"Lease liability - current`"}),2,2)" 'Right-of-use asset'
$ljSchedule = "LeaseJournal$L needs the four rows of LeaseSchedule$L() and the three rows of ROUSchedule$L(), all numbers."
foreach ($r in @(@("{1;2;3}, $lrou(100,1)", $ljSchedule), @("$lsch({105},0.05), $lrou(100,0)", $ljSchedule),
                 @("$lsch({105},0.05), {1;2}", $ljSchedule),
                 @("$lsch({105},0.05), $lrou(100,1), SEQUENCE(1,7)", "LeaseJournal$L takes at most six account names."))) {
    Same "LeaseJournal refuses ($($r[0]))" "LET(x, $lj($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}

# --- Finance comparison: present values pre-tax
Near 'FinanceCompare equals the lease liability in arrears' "INDEX($fc({0,100,100,100},0.05,1),2,3)" "$lli({100,100,100},0.05)"
Near 'FinanceCompare: column 1 is undiscounted' "INDEX($fc({100,100,100},0.05,1),2,3)" '100+NPV(0.05,100,100)'
Near 'FinanceCompare converts the annual rate per period' "INDEX($fc({0,100},0.12),2,3)" '100/1.12^(1/12)'
Near 'FinanceCompare totals each row undiscounted' "INDEX($fc({1000,0,0,0;0,350,350,350},0.05,1),3,2)" '1050'
Near 'FinanceCompare: a receipt is negative' "INDEX($fc({1000,0,-500},0.1,1),2,3)" '1000-500/1.21'
Near 'FinanceCompare shape' "ROWS($fc({1;2;3},0.05))*10+COLUMNS($fc({1;2;3},0.05))" '43'
Same 'FinanceCompare says it is pre-tax' "INDEX($fc({1},0.05),1,3)" 'Present value (pre-tax)'
# Payments in advance start in column 1; laid out as for arrears they are discounted a period
# too far, so the help must say where they go.
Near 'FinanceCompare help places payments in advance' "--ISNUMBER(SEARCH(`"in advance`", TEXTJOIN(`" `", TRUE, $fc())))" '1'
# A restoration estimate in the asset's cost (AASB 16 paragraph 24(d)) is a provision under
# AASB 137, not a cash payment, so the help must not send it to cash.
Near 'LeaseJournal help keeps a restoration estimate out of cash' "--ISNUMBER(SEARCH(`"24(b) to (d)`", TEXTJOIN(`" `", TRUE, $lj())))" '0'
Near 'LeaseJournal help names the restoration provision' "--ISNUMBER(SEARCH(`"AASB 137`", TEXTJOIN(`" `", TRUE, $lj())))" '1'
Same 'FinanceCompare labels the options' "INDEX($fc({1;2},0.05,1,{`"Cash`";`"Lease`"}),3,1)" 'Lease'
Same 'FinanceCompare default labels' "INDEX($fc({1;2},0.05),3,1)" 'Option 2'
Same 'FinanceCompare: a blank Labels cell keeps the default labels' "INDEX($fc({1;2},0.05,1,Z1),3,1)" 'Option 2'
Same 'FinanceCompare keeps the default for a blank label' "INDEX($fc({1;2},0.05,1,{`"Cash`";`"`"}),3,1)" 'Option 2'
Near 'FinanceCompare blank optional cells are the defaults' "SUM(DROP($fc({0,100},0.12,Z1),1,1)-DROP($fc({0,100},0.12),1,1))" '0'
$fcFlows = "FinanceCompare$L needs every cash flow to be a number or blank."
$fcRate = "FinanceCompare$L needs DiscountRate to be a single number above -1 and PeriodsPerYear a single number above 0."
foreach ($r in @(@('{100,"x"}, 0.05', $fcFlows), @('{100,TRUE}, 0.05', $fcFlows), @('{100,100}, -1', $fcRate),
                 @('{100,100}, "5%"', $fcRate), @('{100,100}, {0.05,0.06}', $fcRate), @('{100,100}, 0.05, 0', $fcRate),
                 @('{100;100}, 0.05, 1, {"Cash"}', "FinanceCompare$L needs one label for each row of CashFlows."))) {
    Same "FinanceCompare refuses ($($r[0]))" "LET(x, $fc($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}

# --- CorkscrewRate. Interest is on the average balance and added to it. The check solves
# each period again by fixed-point iteration, I = r * (2 * O + F + I) / 2, rather than the
# closed form, and every column must add up: opening + flows + interest = closing.
$crr = "oz.CorkscrewRate$L"
$crrRun = "$crr(25000, $lpR, SEQUENCE(1, 24, 500, 25), SEQUENCE(1, 24, -300, 0))"
Near 'CorkscrewRate: every column adds up' `
     "LET(s, $crrRun, n, ROWS(s), MAX(ABS(INDEX(s, n, 0) - INDEX(s, 1, 0) - INDEX(s, 2, 0) - INDEX(s, 3, 0) - INDEX(s, 4, 0))))" '0' '0.0000001'
Near 'CorkscrewRate: each opening is the last closing' `
     "LET(s, $crrRun, MAX(ABS(DROP(INDEX(s, 1, 0), , 1) - DROP(INDEX(s, 5, 0), , -1))))" '0' '0.0000001'
Near 'CorkscrewRate: closings agree with an iterated solve' `
     "LET(r, $lpR, f, SEQUENCE(1, 24, 500, 25) + SEQUENCE(1, 24, -300, 0), ref, SCAN(25000, SEQUENCE(1, 24), LAMBDA(b, k, LET(rr, INDEX(r, 1, k), ff, INDEX(f, 1, k), b + ff + REDUCE(0, SEQUENCE(60), LAMBDA(i, n, rr * (2 * b + ff + i) / 2))))), MAX(ABS(INDEX($crrRun, 5, 0) - ref)))" '0' '0.0000001'
Near 'CorkscrewRate: first interest by hand'     "INDEX($crr(1000, 0.01, {100,100}), 3, 1)" '2210/1.99 - 1100' '0.0000000001'
Near 'CorkscrewRate: one rate is every period'   "SUM(ABS($crr(1000, 0.01, {100,200,300}) - $crr(1000, {0.01,0.01,0.01}, {100,200,300})))" '0'
Near 'CorkscrewRate: nil rate is Corkscrew'      "SUM(ABS(TAKE($crr(1000, 0, {100,200,300}), -1) - TAKE(oz.Corkscrew$L(1000, {100,200,300}), -1)))" '0'
Near 'CorkscrewRate: one period'                 "INDEX($crr(1000, 0.01, 100), 4, 1)" '2210/1.99' '0.0000000001'
Near 'CorkscrewRate: range flows match an array' "SUM(ABS($crr(100, 0.01, AE1:AH1) - $crr(100, 0.01, {1,2,3,4})))" '0'
# Z1 is blank and Y1:Z1 holds 0 then a blank: a missing rate is refused, not read as nil.
# AI1:AL1 holds 3 blanks then the text TBA. SUM leaves text and TRUE out of a total, so a
# flow typed as text would show in the flows row but not reach the closing balance.
$crShape   = "CorkscrewRate$L needs Rates to be one rate, or one row with a rate for each period."
$crRate    = "CorkscrewRate$L needs every rate to be a number above -1 and below 2."
$crOpening = "CorkscrewRate$L needs Opening to be a single number."
$crFlow    = "CorkscrewRate$L needs every flow to be a number or blank."
foreach ($r in @(@('100, {0.01,2}, {1,2}', $crRate), @('100, {0.01,0.01,0.01}, {1,2}', $crShape),
                 @('100, "0.01", {1,2}', $crRate), @('100, {0.01;0.01}, {1,2}', $crShape), @('100, -1, {1,2}', $crRate),
                 @('100, Z1, {1,2}', $crRate), @('100, Y1:Z1, {1,2}', $crRate), @('"100", 0.01, {1,2}', $crOpening),
                 @('AE1:AF1, 0.01, {1,2}', $crOpening), @('TRUE, 0.01, {1,2}', $crOpening),
                 @('100, 0.01, {1,"2"}', $crFlow), @('100, 0.01, {1,2}, {TRUE,0}', $crFlow), @('100, 0.01, AI1:AL1', $crFlow))) {
    Same "CorkscrewRate refuses ($($r[0]))" "LET(x, $crr($($r[0])), IF(AND(ROWS(x) = 1, COLUMNS(x) = 1), x, `"array`"))" $r[1]
}
Near 'CorkscrewRate: a blank Opening is nil'     "SUM(ABS($crr(Z1, 0.01, {1,2}) - $crr(0, 0.01, {1,2})))" '0'
Near 'CorkscrewRate: an empty-text flow is nil'  "SUM(ABS(TAKE($crr(100, 0.01, {1,`"`"}), -1) - TAKE($crr(100, 0.01, {1,0}), -1)))" '0'
Near 'CorkscrewRate: an error in a flow shows'   "--ISNA(INDEX($crr(100, 0.01, {1,#N/A}), 4, 2))" '1'
Near 'CorkscrewRate: an error in Rates shows' "--ISNA(INDEX($crr(100, {0.01,#N/A}, {1,2}), 4, 2))" '1'
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
# A column of units is read row by row: one blank defaults its own row to days.
Near 'DateDif: per-row unit'           "INDEX($dd(DATE(2026,1,31), DATE(2026,3,1), {`"MD`";`"`"}),1,1)" '1'
Near 'DateDif: blank unit row is days' "INDEX($dd(DATE(2026,1,31), DATE(2026,3,1), {`"MD`";`"`"}),2,1)" '29'

# --- IsOccurrenceDate. Repeats and LastOccurrence are table columns, so each row is read
# on its own: a blank Repeats row is one-off, a text last date is read as a date, and a
# blank last date means no end. OR() once let one row decide how every row was read.
# 46211 is 8 July 2026, the date tested; the items start on 1 July.
$io = "oz.IsOccurrenceDate$L"
Near 'IsOccurrenceDate: per-row repeats'         "--INDEX($io(DATE(2026,7,8), DATE(2026,7,1), , {`"W`";`"`"}),1,1)" '1'
Near 'IsOccurrenceDate: blank repeats is one-off' "--INDEX($io(DATE(2026,7,8), DATE(2026,7,1), , {`"W`";`"`"}),2,1)" '0'
Near 'IsOccurrenceDate: text last date is read'  "--INDEX($io(DATE(2026,7,8), DATE(2026,7,1), {`"2026-07-05`";46211;`"`"}, `"D`"),1,1)" '0'
Near 'IsOccurrenceDate: date last date row'      "--INDEX($io(DATE(2026,7,8), DATE(2026,7,1), {`"2026-07-05`";46211;`"`"}, `"D`"),2,1)" '1'
Near 'IsOccurrenceDate: blank last date row'     "--INDEX($io(DATE(2026,7,8), DATE(2026,7,1), {`"2026-07-05`";46211;`"`"}, `"D`"),3,1)" '1'
Near 'IsOccurrenceDate: blank last date cells'   "SUM(--$io(DATE(2026,7,8), DATE(2026,7,1), Z1:Z2, `"D`"))" '2'

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
# principal. InterestLRV solves that in closed form; the check allows 0.001.
Near 'Debt: LRV on PeriodRates, first interest'   "LET(r, 0.073*31/365, INDEX($lrv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, , , $rates),2,1) - r*(1000-125)/(1-r/2))" '0' '0.001'
Near 'Debt: blank PeriodRates cell is not given'  "SUMPRODUCT(ABS($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12, Z1) - $dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt: blank PeriodRates range is not given' "SUMPRODUCT(ABS($dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12, Z1:Z3) - $dsv(, {1000,0,0}, {300,300,300}, {1.2,1.2,1.2}, {0.06,0.06,0.06}, 12)))" '0' '0.0000001'
Near 'Debt: LRV on PeriodRates ends at zero'      "INDEX($lrv(, {1000,0,0}, {1800,1800,1800}, {1.2,1.2,1.2}, , , $rates),4,1)" '0' '0.0000001'

# --- Working-capital days. A zero flow is Excel's #DIV/0!, as for every ratio; Days of 0
# or less is #NUM!; a blank Days or WIPDays cell, or an empty string, takes the default
# for its own row; any other Days that is not a number is #VALUE!, numeric text included,
# since "0" once slipped past the Days check. ERROR.TYPE: 2 is #DIV/0!, 3 #VALUE!, 6 #NUM!.
$rd = "oz.ReceivableDays$L"
$pd = "oz.PayableDays$L"
$wd = "oz.WIPDays$L"
$cc = "oz.CashConversionCycle$L"
Near 'ReceivableDays: nothing owed is 0 days'       "$rd(0, 1200000)"                          '0'
Near 'ReceivableDays: zero sales is #DIV/0!'        "ERROR.TYPE($rd(125000, 0))"               '2' '0.1'
Near 'ReceivableDays: negative sales flip the sign' "$rd(125000, -1200000)"                    '-38.0208333333' '0.0000001'
Near 'ReceivableDays: zero Days is #NUM!'           "ERROR.TYPE($rd(125000, 1200000, 0))"      '6' '0.1'
Near 'ReceivableDays: negative Days is #NUM!'       "ERROR.TYPE($rd(125000, 1200000, -90))"    '6' '0.1'
Near 'ReceivableDays: blank Days cell is 365'       "$rd(125000, 1200000, Z1)"                 '38.0208333333' '0.0000001'
Near 'ReceivableDays: numeric text Days is #VALUE!' "ERROR.TYPE($rd(125000, 1200000, `"0`"))"  '3' '0.1'
Near 'ReceivableDays: row by row'                   "SUMPRODUCT(ABS($rd({100,200},{1000,4000},{365,90}) - {36.5,4.5}))" '0' '0.0000001'
Near 'ReceivableDays: blank Days row is 365'        "INDEX($rd({100;100},{1000;1000},{90;`"`"}),2,1)" '36.5' '0.0000001'
Near 'ReceivableDays: 365 over the turnover ratio'  "$rd(125000, 1200000) - 365/oz.ReceivablesTurnoverRatio$L(1200000, 125000)" '0' '0.0000001'
Near 'PayableDays: blank Days cell is 365'          "$pd(64000, 780000, Z1)"                   '29.9487179487' '0.0000001'
Near 'PayableDays: zero Days is #NUM!'              "ERROR.TYPE($pd(64000, 780000, 0))"        '6' '0.1'
Near 'WIPDays: a month of fees'                     "$wd(42000, 45000, 30)"                    '28' '0.0000001'
Near 'WIPDays: zero flow is #DIV/0!'                "ERROR.TYPE($wd(42000, 0))"                '2' '0.1'
Near 'CashConversionCycle: without WIP'             "$cc(40, 30, 25)"                          '45'
Near 'CashConversionCycle: with WIP'                "$cc(40, 30, 25, 10)"                      '55'
Near 'CashConversionCycle: blank WIPDays cell is 0' "$cc(40, 30, 25, Z1)"                      '45'
Near 'CashConversionCycle: suppliers paid last'     "$cc(10, 5, 30)"                           '-15'
Near 'CashConversionCycle: an error propagates'     "--ISERROR($cc(NA(), 30, 25))"             '1'
Near 'CashConversionCycle: composes the parts'      "$cc(oz.DSI$L(54.9, 490), $rd(125000, 1200000), $pd(64000, 780000))" '48.9670133438' '0.0000001'

# --- ATO benchmark ratios and the dated rate lookup. Turnover falls back to total
# business income when other sales are nil, blank or under half of it, and exactly half
# is not under half. Blank optional amounts count as 0. RateAtDate refuses an incomplete,
# unsorted or repeating table with a message, and a date before the first effective date
# is #N/A. ERROR.TYPE: 2 is #DIV/0!, 3 #VALUE!, 7 #N/A.
$atu = "oz.ATOTurnover$L"
$abr = "oz.ATOBenchmarkRatio$L"
$ate = "oz.ATOTotalExpensesRatio$L"
$acs = "oz.ATOCostOfSalesRatio$L"
$alr = "oz.ATOLabourRatio$L"
$rad = "oz.RateAtDate$L"
$radTable = "DATE(2026,{1;7;10},1), {0.04;0.045;0.05}"
Near 'ATOTurnover: exactly half keeps other sales'  "$atu(250000, 500000)"                     '250000'
Near 'ATOTurnover: nil other sales'                 "$atu(0, 500000)"                          '500000'
Near 'ATOTurnover: blank other sales cell'          "$atu(Z1, 500000)"                         '500000'
Near 'ATOTurnover: text income is #VALUE!'          "ERROR.TYPE($atu(480000, `"500000`"))"     '3' '0.1'
Near 'ATOTurnover: row by row'                      "SUMPRODUCT(ABS($atu({480000,200000},{500000,500000}) - {480000,500000}))" '0'
Near 'ATOBenchmarkRatio: zero turnover is #DIV/0!'  "ERROR.TYPE($abr(24000, 0))"               '2' '0.1'
Near 'ATOTotalExpensesRatio: blank associates'      "$ate(410000, 480000, Z1)"                 '0.8541666667' '0.0000001'
Near 'ATOCostOfSalesRatio: wages not in cost'       "$acs(260000, 480000)"                     '0.5416666667' '0.0000001'
Near 'ATOLabourRatio: W1 below wages is ignored'    "$alr(90000, 20000, 480000, 10000, 85000)" '0.2083333333' '0.0000001'
Near 'ATOLabourRatio: W1 row by row'                "SUMPRODUCT(ABS($alr({100,100}, 0, {1000,1000}, 0, {150,50}) - {0.15,0.1}))" '0' '0.0000001'
Near 'RateAtDate: on an effective date'             "$rad($radTable, DATE(2026,7,1))"          '0.045' '0.0000001'
Near 'RateAtDate: after the last date'              "$rad($radTable, DATE(2030,1,1))"          '0.05' '0.0000001'
Near 'RateAtDate: before the first date is #N/A'    "ERROR.TYPE($rad($radTable, DATE(2025,12,31)))" '7' '0.1'
Near 'RateAtDate: a row table and a row of dates'   "SUMPRODUCT(ABS($rad(DATE(2026,{1,7,10},1), {0.04,0.045,0.05}, DATE(2026,{2,8,11},1)) - {0.04,0.045,0.05}))" '0' '0.0000001'
Near 'RateAtDate: text date is #VALUE!'             "ERROR.TYPE($rad($radTable, `"2026-08-01`"))" '3' '0.1'
Near 'RateAtDate: blank rate cells are a gap'       "--ISNUMBER(SEARCH(`"gap in the table`", $rad(DATE(2026,{1;7},1), Z1:Z2, DATE(2026,8,1))))" '1'
Near 'RateAtDate: lengths that differ are a gap'    "--ISNUMBER(SEARCH(`"gap in the table`", $rad(DATE(2026,{1;7;10},1), {0.04;0.045}, DATE(2026,8,1))))" '1'
Near 'RateAtDate: unsorted dates'                   "--ISNUMBER(SEARCH(`"ascending order`", $rad(DATE(2026,{7;1},1), {0.04;0.045}, DATE(2026,8,1))))" '1'
Near 'RateAtDate: repeated date'                    "--ISNUMBER(SEARCH(`"repeats`", $rad(DATE(2026,{7;7},1), {0.04;0.045}, DATE(2026,8,1))))" '1'

# --- Variance and financial-year periods. A favourable variance is positive for income
# (Type 1) and expenses (Type -1); the percentage divides by the budget's size, so a
# negative budget keeps the variance's sign. A blank inside a column of Types is refused,
# and so is TRUE, which ABS would read as 1. YearToDate counts columns dated from the
# first day of AsAt's financial year to AsAt. Months and quarters count from StartMonth.
# ERROR.TYPE: 2 is #DIV/0!, 3 #VALUE!.
$var = "oz.Variance$L"
$vpc = "oz.VariancePct$L"
$ytd = "oz.YearToDate$L"
$fmo = "oz.FinancialMonth$L"
$fqu = "oz.FinancialQuarter$L"
$ytdDates = "DATE(2026,{5,6,7,8},1)"
Near 'Variance: income above budget'                "$var(120, 100)"                           '20'
Near 'Variance: expense below budget'               "$var(90, 100, -1)"                        '10'
Near 'Variance: expense above budget'               "$var(110, 100, -1)"                       '-10'
Near 'Variance: one Type for each line'             "SUMPRODUCT(ABS($var({120;90},{100;100},{1;-1}) - {20;10}))" '0'
Near 'Variance: a single blank Type is income'      "$var(120, 100, Z1)"                       '20'
Near 'Variance: blank Types in a column'            "--ISNUMBER(SEARCH(`"1 for income`", $var({120;90}, {100;100}, Z1:Z2)))" '1'
Near 'Variance: Type 2 is refused'                  "--ISNUMBER(SEARCH(`"1 for income`", $var(120, 100, 2)))" '1'
Near 'Variance: Type TRUE is refused'               "--ISNUMBER(SEARCH(`"1 for income`", $var(120, 100, TRUE)))" '1'
Near 'Variance: text Type is refused'               "--ISNUMBER(SEARCH(`"1 for income`", $var(120, 100, `"1`")))" '1'
Near 'Variance: text actual is refused'            "--ISNUMBER(SEARCH(`"numbers or blank`", $var(`"x`", 100)))" '1'
Near 'VariancePct: income above budget'             "$vpc(110, 100)"                           '0.1'
Near 'VariancePct: expense above budget'            "$vpc(110, 100, -1)"                       '-0.1'
Near 'VariancePct: negative budget, unfavourable'   "$vpc(-30, -20)"                           '-0.5'
Near 'VariancePct: negative budget, favourable'     "$vpc(-10, -20)"                           '0.5'
Near 'VariancePct: zero budget is #DIV/0!'          "ERROR.TYPE($vpc(5, 0))"                   '2' '0.1'
Near 'VariancePct: Type 0 is refused'               "--ISNUMBER(SEARCH(`"1 for income`", $vpc(110, 100, 0)))" '1'
Near 'YearToDate: help example, row by row'         "SUMPRODUCT(ABS($ytd({10,20,30,40;1,2,3,4}, $ytdDates, DATE(2026,8,31)) - {70;7}))" '0'
Near 'YearToDate: June closes the earlier year'     "$ytd({10,20,30,40}, $ytdDates, DATE(2026,6,30))" '30'
Near 'YearToDate: first month of the year'          "$ytd({10,20,30,40}, $ytdDates, DATE(2026,7,31))" '30'
Near 'YearToDate: a mid-month AsAt'                 "$ytd({10,20,30,40}, $ytdDates, DATE(2026,8,15))" '70'
Near 'YearToDate: calendar year'                    "$ytd({10,20,30,40}, $ytdDates, DATE(2026,8,31), 1)" '100'
Near 'YearToDate: blank values count as 0'          "$ytd(Z1:AC1, $ytdDates, DATE(2026,8,31))" '0'
Near 'YearToDate: Dates must match Values'          "--ISNUMBER(SEARCH(`"one row with a date`", $ytd({10,20,30,40}, DATE(2026,{5,6,7},1), DATE(2026,8,31))))" '1'
Near 'YearToDate: text in Dates'                    "--ISNUMBER(SEARCH(`"one row with a date`", $ytd({10,20}, {`"x`",46000}, DATE(2026,8,31))))" '1'
Near 'YearToDate: AsAt must be one date'            "--ISNUMBER(SEARCH(`"one date`", $ytd({10,20,30,40}, $ytdDates, DATE(2026,{7,8},31))))" '1'
Near 'YearToDate: StartMonth 13 is refused'         "--ISNUMBER(SEARCH(`"1 to 12`", $ytd({10,20,30,40}, $ytdDates, DATE(2026,8,31), 13)))" '1'
Near 'FinancialMonth: July is month 1'              "$fmo(DATE(2026,7,1))"                     '1'
Near 'FinancialMonth: June is month 12'             "$fmo(DATE(2027,6,30))"                    '12'
Near 'FinancialMonth: April start, March'           "$fmo(DATE(2026,3,31), 4)"                 '12'
Near 'FinancialMonth: calendar year'                "$fmo(DATE(2026,1,15), 1)"                 '1'
Near 'FinancialMonth: blank StartMonth cell'        "$fmo(DATE(2026,7,1), Z1)"                 '1'
Near 'FinancialMonth: a row of dates'               "SUMPRODUCT(ABS($fmo(DATE(2026,{6,7,12},1)) - {12,1,6}))" '0'
Same 'FinancialMonth: blank is blank'               "$fmo(Z1)"                                 ''
Near 'FinancialMonth: StartMonth 6.5 is refused'    "--ISNUMBER(SEARCH(`"1 to 12`", $fmo(DATE(2026,7,1), 6.5)))" '1'
Near 'FinancialQuarter: September is quarter 1'     "$fqu(DATE(2026,9,30))"                    '1'
Near 'FinancialQuarter: October is quarter 2'       "$fqu(DATE(2026,10,1))"                    '2'
Near 'FinancialQuarter: June is quarter 4'          "$fqu(DATE(2027,6,30))"                    '4'
Near 'FinancialQuarter: calendar year, April'       "$fqu(DATE(2026,4,1), 1)"                  '2'
Near 'FinancialQuarter: StartMonth 0 is refused'    "--ISNUMBER(SEARCH(`"1 to 12`", $fqu(DATE(2026,7,1), 0)))" '1'
# Inputs that arithmetic and broadcasting would otherwise turn into plausible numbers:
# a Type row against a column of amounts, amounts of different shapes, text and logical
# amounts, a range of start months, a time of day on a column date, and a year that
# would start before 1900 (DATE reads 1899 as 3799; EDATE gives #NUM!, ERROR.TYPE 6).
Near 'Variance: a Type row is a shape error'        "--ISNUMBER(SEARCH(`"same shape`", $var({120;90}, {100;100}, {1,-1})))" '1'
Near 'Variance: amounts of different shapes'        "--ISNUMBER(SEARCH(`"same shape`", $var({120;90}, {100,80})))" '1'
Near 'Variance: too many Types'                     "--ISNUMBER(SEARCH(`"same shape`", $var({120;90}, {100;100}, {1;-1;1})))" '1'
Near 'Variance: a Type for each row and column'     "SUMPRODUCT(ABS($var({120,130;90,80}, {100,100;100,100}, {1,1;-1,-1}) - {20,30;10,20}))" '0'
Near 'Variance: one Type per row across months'     "SUMPRODUCT(ABS($var({120,130;90,80}, {100,100;100,100}, {1;-1}) - {20,30;10,20}))" '0'
Near 'Variance: TRUE amount is refused'             "--ISNUMBER(SEARCH(`"numbers or blank`", $var(TRUE, 0)))" '1'
Near 'Variance: text amount is refused'             "--ISNUMBER(SEARCH(`"numbers or blank`", $var(`"120`", 100)))" '1'
Near 'Variance: blank actual cell counts as 0'      "$var(Z1, 100)"                            '-100'
Near 'Variance: an error amount shows'              "ERROR.TYPE($var(NA(), 100))"             '7' '0.1'
Near 'VariancePct: FALSE amount is refused'         "--ISNUMBER(SEARCH(`"numbers or blank`", $vpc(FALSE, 100)))" '1'
Near 'VariancePct: a Type row is a shape error'     "--ISNUMBER(SEARCH(`"same shape`", $vpc({120;90}, {100;100}, {1,-1})))" '1'
Near 'FinancialMonth: a range of starts refused'    "--ISNUMBER(SEARCH(`"1 to 12`", $fmo(DATE(2026,7,1), {7,13})))" '1'
Near 'FinancialQuarter: a range of starts refused'  "--ISNUMBER(SEARCH(`"1 to 12`", $fqu(DATE(2026,7,1), {1;7})))" '1'
Near 'YearToDate: a range of starts refused'        "--ISNUMBER(SEARCH(`"1 to 12`", $ytd({10,20}, DATE(2026,{2,5},1), DATE(2026,8,31), {1,7})))" '1'
Near 'YearToDate: text values are refused'          "--ISNUMBER(SEARCH(`"numbers or blank`", $ytd({`"10`",20}, DATE(2026,{7,8},1), DATE(2026,8,31))))" '1'
Near 'YearToDate: TRUE values are refused'          "--ISNUMBER(SEARCH(`"numbers or blank`", $ytd({TRUE,20}, DATE(2026,{7,8},1), DATE(2026,8,31))))" '1'
# The error is in an array constant: HSTACK, like VSTACK, returns #N/A whole when an argument is NA().
Near 'YearToDate: an error in an earlier column'    "ERROR.TYPE($ytd({#N/A,20}, DATE(2026,{6,8},1), DATE(2026,8,31)))" '7' '0.1'
Near 'YearToDate: a time of day is ignored'         "$ytd({10,20}, DATE(2026,8,31) + {0,0.5}, DATE(2026,8,31))" '30'
Near 'YearToDate: a year before 1900 is #NUM!'      "ERROR.TYPE($ytd({10,20}, DATE(1900,{1,6},1), DATE(1900,6,30)))" '6' '0.1'
Near 'YearToDate: a 1900 calendar year'             "$ytd({10,20}, DATE(1900,{1,6},1), DATE(1900,6,30), 1)" '30'
Near 'Variance: a block of blank cells is 0'       "SUMPRODUCT(ABS($var(Z1:AA2, {100,100;100,100}) + 100))" '0'
Near 'YearToDate: blank and numbers in a block'     "SUMPRODUCT(ABS($ytd(HSTACK(Z1:Z2, {20;30}), DATE(2026,{7,8},1), DATE(2026,8,31)) - {20;30}))" '0'
Near 'FinancialMonth: text StartMonth is refused'   "--ISNUMBER(SEARCH(`"1 to 12`", $fmo(DATE(2026,7,1), `"7`")))" '1'
Near 'FinancialQuarter: TRUE StartMonth refused'    "--ISNUMBER(SEARCH(`"1 to 12`", $fqu(DATE(2026,7,1), TRUE)))" '1'
Near 'YearToDate: StartMonth 7.5 is refused'        "--ISNUMBER(SEARCH(`"1 to 12`", $ytd({10,20}, DATE(2026,{7,8},1), DATE(2026,8,31), 7.5)))" '1'

# --- Worksheet ranges and input shapes. The generated help examples pass array constants
# and SEQUENCE, which never show a function that mishandles a real range, and nothing
# checked what a column or a block did to a function written for a row. The scratch sheet
# holds 1-4 in AE1:AH1, 10-40 in AE2:AH2, the dates 30 Jun, 1 Jul, 15 Jul and 1 Sep 2026
# in AE3:AH3, period starts 1 Jul and 1 Aug 2026 in AE4:AF4 with month ends in AE5:AF5,
# 1-3 down AN1:AN3, the dates 30 Jun 2026, 31 Jan 2027 and 29 Feb 2028 down AP1:AP3, and
# TBA in AL1.
$cu  = "oz.Cumulative$L"
$sp  = "oz.SumPeriods$L"
$dsi = "oz.DSI$L"
$apA = 'DATE({2026;2027;2028},{6;1;2},{30;31;29})'
$e3A = 'DATE(2026,{6,7,7,9},{30,1,15,1})'
$e4A = 'DATE(2026,{7,8},1)'
$e5A = 'DATE(2026,{7,8},31)'
# A range must give exactly what the same values give as an array, and neither may be
# one of the refusal messages, which all say what an argument must be.
# The same shape and the same values, and not a refusal message.
function SameAsArray($id, $ranged, $arrayed) {
    Near $id "LET(x, $ranged, y, $arrayed, r, TEXTJOIN(`"|`",FALSE,x), a, TEXTJOIN(`"|`",FALSE,y), --AND(ROWS(x)=ROWS(y), COLUMNS(x)=COLUMNS(y), r=a, ISERROR(SEARCH(`"must`", r))))" '1'
}

# Cumulative ran SCAN over a block row by row, so each row opened on the previous row's
# closing total. A row and a column are the shapes its help names; a block is refused.
Near 'Cumulative: a row range'                   "SUMPRODUCT(ABS($cu(AE1:AH1) - {1,3,6,10}))" '0'
Near 'Cumulative: a column range'                "SUMPRODUCT(ABS($cu(AN1:AN3) - {1;3;6}))" '0'
Near 'Cumulative: an opening balance on a range' "INDEX($cu(AE1:AH1, 100), 1, 4)" '110'
Near 'Cumulative: a block range is refused'      "--ISNUMBER(SEARCH(`"one row or one column`", $cu(AE1:AH2)))" '1'
Near 'Cumulative: a block array is refused'      "--ISNUMBER(SEARCH(`"one row or one column`", $cu({1,2;3,4})))" '1'

# The rolling functions window over columns. A column came back as one total of the whole
# column and a block as its rows added together, both looking like an answer.
foreach ($r in @(@('Sum', 'AE1:AH1', '{1,3,5,7}'), @('Avg', 'AE1:AH1', '{1,1.5,2.5,3.5}'),
                 @('Max', 'AE2:AH2', '{10,20,30,40}'), @('Min', 'AE2:AH2', '{10,10,20,30}'))) {
    $fn = "oz.Rolling$($r[0])$L"
    Near "Rolling$($r[0]): a row range"         "SUMPRODUCT(ABS($fn($($r[1]), 2) - $($r[2])))" '0'
    Near "Rolling$($r[0]): a column is refused" "--ISNUMBER(SEARCH(`"must be one row`", $fn(AN1:AN3, 2)))" '1'
    Near "Rolling$($r[0]): the column message names TRANSPOSE" "--ISNUMBER(SEARCH(`"TRANSPOSE`", $fn(AN1:AN3, 2)))" '1'
    Near "Rolling$($r[0]): a block is refused"  "--ISNUMBER(SEARCH(`"must be one row`", $fn(AE1:AH2, 2)))" '1'
    Near "Rolling$($r[0]): a calculated column is refused" "--ISNUMBER(SEARCH(`"must be one row`", $fn(SEQUENCE(3), 2)))" '1'
    Near "Rolling$($r[0]): a calculated block is refused"  "--ISNUMBER(SEARCH(`"must be one row`", $fn(SEQUENCE(2,2), 2)))" '1'
}
Near 'RollingSum: a transposed column, as the message advises' "SUMPRODUCT(ABS(oz.RollingSum$L(TRANSPOSE(AN1:AN3), 2) - {1,3,5}))" '0'

# SumPeriods leaves out a date before the first period start and gives a date after the
# last start to the last period, as its help now says. Pin both, on ranges.
Near 'SumPeriods: range inputs'                         "SUMPRODUCT(ABS($sp(AE1:AH2, AE3:AH3, AE4:AF4) - {5,4;50,40}))" '0'
Near 'SumPeriods: a date before the first start is left out' "SUM(AE1:AH2) - SUM($sp(AE1:AH2, AE3:AH3, AE4:AF4))" '11'
Near 'SumPeriods: a later date goes to the last period' "INDEX($sp(AE1:AH1, AE3:AH3, AE4:AF4), 1, 2)" '4'
Near 'SumPeriods: a date on the last start goes to the last period' `
     "SUMPRODUCT(ABS($sp({10,20,30}, DATE(2026,{6,7,8},{30,1,1}), DATE(2026,{7,8},1)) - {20,30}))" '0'

# Depreciate read a salvage value it could not parse as 0, so an asset whose salvage was
# still TBA depreciated to nil without a word.
$dpTl = 'EDATE(DATE(2026,7,1), SEQUENCE( , 60, 0))'
$dpS  = "1000, DATE(2026,7,1), 5, $dpTl"
Near 'Depreciate: text salvage is refused'          "--ISNUMBER(SEARCH(`"SalvageValues must be`", $dp($dpS, `"TBA`")))" '1'
Near 'Depreciate: text salvage in a cell is refused' "--ISNUMBER(SEARCH(`"SalvageValues must be`", $dp($dpS, AL1)))" '1'
Near 'Depreciate: a blank salvage cell is 0'        "SUMPRODUCT(ABS($dp($dpS, Z1) - $dp($dpS)))" '0'
# Text is refused even when it reads as a number, since VALUE also reads date text as a serial.
foreach ($t in '100', '1/1/2026', '12:00') {
    Near "Depreciate: salvage text $t is refused" "--ISNUMBER(SEARCH(`"SalvageValues must be`", $dp($dpS, `"$t`")))" '1'
}
Near 'Depreciate: a salvage amount still applies' `
     "SUM(INDEX($dp($dpS, 0), 3, 0)) - SUM(INDEX($dp($dpS, 100), 3, 0))" '100' '0.005'
# An error salvage gets the message as well. Passed through, it made the whole schedule one
# #VALUE!, whichever error it was and however many assets were fine.
Near 'Depreciate: an error salvage is refused' "--ISNUMBER(SEARCH(`"SalvageValues must be`", $dp($dpS, NA())))" '1'
Near 'Depreciate: one asset''s error salvage is refused' `
     "--ISNUMBER(SEARCH(`"SalvageValues must be`", $dp({1000;1000}, DATE(2026,7,1)*{1;1}, {5;5}, $dpTl, {100;#DIV/0!})))" '1'
# More than one asset needs one life per asset; a single life then returns #VALUE!. A
# multi-asset schedule separates its assets with text, so compare it as text.
SameAsArray 'Depreciate: range inputs equal arrays' `
     "$dp(AN1:AN2, AP2:AP3, {5;5}, $dpTl)" "$dp({1;2}, DATE({2027;2028},{1;2},{31;29}), {5;5}, $dpTl)"

# DSI fixed 365 days where the other day ratios take Days.
Near 'DSI: Days defaults to 365'      "$dsi(54.9, 490)"         '54.9/490*365'
Near 'DSI: Days of 366'               "$dsi(54.9, 490, 366)"    '54.9/490*366'
Near 'DSI: a quarter'                 "$dsi(54.9, 120, 90)"     '54.9/120*90'
Near 'DSI: a blank Days cell is 365'  "$dsi(54.9, 490, Z1)"     '54.9/490*365'
Near 'DSI: an empty Days is 365'      "$dsi(54.9, 490, `"`")" '54.9/490*365'
Near 'DSI: negative Days is #NUM!'    "ERROR.TYPE($dsi(54.9, 490, -1))"     '6'
Near 'DSI: an error Days passes through' "ERROR.TYPE($dsi(54.9, 490, NA()))" '7'
Near 'DSI: a #DIV/0! Days passes through' "ERROR.TYPE($dsi(54.9, 490, 1/0))" '2'
Near 'DSI: word Days is #VALUE!'          "ERROR.TYPE($dsi(54.9, 490, `"TBA`"))" '3'
Near 'DSI: Days of 0 is #NUM!'        "ERROR.TYPE($dsi(54.9, 490, 0))"      '6'
Near 'DSI: text Days is #VALUE!'      "ERROR.TYPE($dsi(54.9, 490, `"90`"))" '3'

# Every date function given worksheet ranges, against the same dates as calculated arrays.
SameAsArray 'FinancialYear: a range of dates'    "$fy(AP1:AP3)"  "$fy($apA)"
SameAsArray 'FinancialMonth: a range of dates'   "$fmo(AP1:AP3)" "$fmo($apA)"
SameAsArray 'FinancialQuarter: a range of dates' "$fqu(AP1:AP3)" "$fqu($apA)"
SameAsArray 'PeriodLabel: a range of dates'      "oz.PeriodLabel$L(AP1:AP3)" "oz.PeriodLabel$L($apA)"
SameAsArray 'DateDif: ranges of dates'           "oz.DateDif$L(AP1:AP2, AP2:AP3)" `
                                                 "oz.DateDif$L(DATE({2026;2027},{6;1},{30;31}), DATE({2027;2028},{1;2},{31;29}))"
SameAsArray 'PeriodStart: a range of dates'      "oz.PeriodStart$L(AF3, 1, AP1:AP3)" "oz.PeriodStart$L(DATE(2026,7,1), 1, $apA)"
SameAsArray 'IsBetween: a range of dates'        "oz.IsBetween$L(AE3:AH3, AE4, AF4)" "oz.IsBetween$L($e3A, DATE(2026,7,1), DATE(2026,8,1))"
SameAsArray 'IsOccurrenceDate: a range of dates' "oz.IsOccurrenceDate$L(AE3:AH3, AF3)" "oz.IsOccurrenceDate$L($e3A, DATE(2026,7,1))"
SameAsArray 'Periods: ranges of dates'           "oz.Periods$L(AE4:AF4, AE5:AF5)" "oz.Periods$L($e4A, $e5A)"
SameAsArray 'OverLapDays: ranges of dates'       "oz.OverLapDays$L(AE4:AF4, AE5:AF5, DATE(2026,7,15), DATE(2026,8,15))" `
                                                 "oz.OverLapDays$L($e4A, $e5A, DATE(2026,7,15), DATE(2026,8,15))"
SameAsArray 'CountDOW: ranges of dates'          "oz.CountDOW$L(AE4:AF4, AE5:AF5, 1)" "oz.CountDOW$L($e4A, $e5A, 1)"
SameAsArray 'DayCountRate: a range timeline'     "oz.DayCountRate$L(AE4:AF4, 0.073)" "oz.DayCountRate$L($e4A, 0.073)"
SameAsArray 'TimelineOffset: a range timeline'   "oz.TimelineOffset$L(AH3, AE4:AF4)" "oz.TimelineOffset$L(DATE(2026,9,1), $e4A)"
SameAsArray 'Timeline: a start date in a cell'   "oz.Timeline$L(AF3, 3, `"M`")" "oz.Timeline$L(DATE(2026,7,1), 3, `"M`")"
SameAsArray 'RateAtDate: ranges of dates'        "oz.RateAtDate$L(AE4:AF4, {0.05,0.06}, AF3:AH3)" `
                                                 "oz.RateAtDate$L($e4A, {0.05,0.06}, DATE(2026,{7,7,9},{1,15,1}))"
SameAsArray 'ScheduleRates: ranges of dates'     "oz.ScheduleRates$L(AE5:AF5, AE4:AF4, {0.05,0.06})" "oz.ScheduleRates$L($e5A, $e4A, {0.05,0.06})"
SameAsArray 'ScheduleValues: ranges of dates'    "oz.ScheduleValues$L(AE4:AF4, AE5:AF5, {100,200}, AE4:AF4)" `
                                                 "oz.ScheduleValues$L($e4A, $e5A, {100,200}, $e4A)"
SameAsArray 'YearToDate: ranges of dates'        "$ytd(AE1:AH1, AE3:AH3, DATE(2026,8,31))" "$ytd({1,2,3,4}, $e3A, DATE(2026,8,31))"
Near 'YearToDate: a range of dates totals the year to date' "$ytd(AE1:AH1, AE3:AH3, DATE(2026,8,31))" '5'
# RangeToDA and RangeToDAU spill the block that starts at a cell, 1 row and 1 column by default.
SameAsArray 'RangeToDA: a cell spans the columns asked for'  "oz.RangeToDA$L(AE1, , 4)"  '{1,2,3,4}'
SameAsArray 'RangeToDA: rows and columns'                    "oz.RangeToDA$L(AE1, 2, 4)"  '{1,2,3,4;10,20,30,40}'
SameAsArray 'RangeToDAU: a cell spans the columns asked for' "oz.RangeToDAU$L(AE1, , 4)" '{1,2,3,4}'
SameAsArray 'RangeToDAU: rows and columns'                   "oz.RangeToDAU$L(AE1, 2, 4)" '{1,2,3,4;10,20,30,40}'

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
# The 2 rate converters invert each other, and the monthly one agrees with NOMINAL().
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
# Where DATEDIF is right, on start days up to the 28th, the 2 agree.
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
# with C = 300 / 1.2 the cash for debt service, solved in closed form by InterestLRV.
Fuzz 'fuzz: DebtSculptVariable interest = opening * period rate' `
     'd, RANDBETWEEN(1000,100000), r, RANDBETWEEN(1,20)/1000' '"d="&d&" r="&r' `
     "INDEX($dsv(, HSTACK(d,0,0), {300,300,300}, {1.2,1.2,1.2}, , , HSTACK(r,r,r)), 2, 1)" 'd*r' '0.0000001'
Fuzz 'fuzz: DebtSculptVariableLRV interest on the average balance' `
     'd, RANDBETWEEN(1000,100000), r, RANDBETWEEN(1,20)/1000' '"d="&d&" r="&r' `
     "INDEX($lrv(, HSTACK(d,0,0), {300,300,300}, {1.2,1.2,1.2}, , , HSTACK(r,r,r)), 2, 1)" 'r*(d-125)/(1-r/2)' '0.001'
# The minimum yearly repayment is the s 109E(6) formula as the Act prints it.
Fuzz 'fuzz: Div7AMinRepayment = the s 109E(6) formula' `
     'p, RANDBETWEEN(0,10000000), r, RANDBETWEEN(1,1500)/10000, t, RANDBETWEEN(1,250)/10' '"p="&p&" r="&r&" t="&t' `
     "oz.Div7AMinRepayment$L(p, r, t)" '(p*r)/(1-(1/(1+r))^ROUNDUP(t,0))' '0.000001'
# GST under the margin scheme is 1/11 of the margin, and nil when the price does not exceed the base.
Fuzz 'fuzz: MarginSchemeGST = 1/11 of the excess' `
     'p, RANDBETWEEN(0,5000000), b, RANDBETWEEN(0,5000000)' '"p="&p&" b="&b' `
     "oz.MarginSchemeGST$L(p, b)" 'IF(p>b, (p-b)/11, 0)' '0.000001'
# The gross-up factors reduce to 57000/27401 (type 1) and 100/53 (type 2) at 47% FBT and 10% GST.
Fuzz 'fuzz: FBTGrossUp = value x 57000/27401 or 100/53' `
     'v, RANDBETWEEN(0,1000000)/100, t, RANDBETWEEN(1,2)' '"v="&v&" t="&t' `
     "oz.FBTGrossUp$L(v, t)" 'v*IF(t=1, 57000/27401, 100/53)' '0.000001'
# One year of a low-value pool declines by 18.75% of the low-cost amount and 37.5% of the rest.
Fuzz 'fuzz: LowValuePool decline = 0.1875 x low-cost + 0.375 x (opening + low-value)' `
     'o, RANDBETWEEN(0,1000000)/100, c, RANDBETWEEN(0,100000)/100, v, RANDBETWEEN(0,100000)/100' '"o="&o&" c="&c&" v="&v' `
     "INDEX(oz.LowValuePool$L(o, c, v), 2, 1)" '0.1875*c + 0.375*(o + v)' '0.000001'
# A small business pool of $20,000 or more deducts 30% of the opening and 15% of additions.
Fuzz 'fuzz: SmallBusinessPool deduction = 30% x opening + 15% x additions' `
     'o, RANDBETWEEN(2000000,100000000)/100, a, RANDBETWEEN(0,1000000)/100' '"o="&o&" a="&a' `
     "INDEX(oz.SmallBusinessPool$L(o, a), 2, 1)" '0.3*o + 0.15*a' '0.000001'
# The randomised checks rest on MAP() drawing fresh values for every trial. Prove it.
Near 'fuzz: MAP draws fresh values per trial' "--(ROWS(UNIQUE(MAP(SEQUENCE(25), LAMBDA(i, RANDBETWEEN(1,1000000))))) > 1)" '1'

$xl = $null; $wb = $null; $tmp = $null; $excel = $null; $exit = 0

# Excel rejects incoming COM calls while it is mid-calculation (RPC_E_CALL_REJECTED),
# so anything that can arrive at a busy moment gets retried with a backoff.
# Write one block of values to the scratch sheet in a single call. $rows is an array of
# rows, each an array of cell values.
function Set-Block([string]$range, [object[]]$rows) {
    $block = New-Object 'object[,]' $rows.Count, $rows[0].Count
    for ($r = 0; $r -lt $rows.Count; $r++) {
        for ($c = 0; $c -lt $rows[0].Count; $c++) { $block[$r, $c] = $rows[$r][$c] }
    }
    Invoke-Excel { $tmp.Range($range).Value2 = $block }
}

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
. (Join-Path $PSScriptRoot 'excel_process.ps1')

try {
    $xl = New-Object -ComObject Excel.Application
    $excel = Invoke-Excel { Get-ExcelProcess $xl }
    $xl.Visible = $false; $xl.DisplayAlerts = $false
    # msoAutomationSecurityForceDisable (3), not Low (1): the self-test needs no macros.
    $xl.AutomationSecurity = 3; $xl.AskToUpdateLinks = $false; $xl.EnableEvents = $false

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
    Invoke-Excel { $tmp.Range('Y1').Value2 = 0 }          # numeric cash beside a real blank
    # Worksheet ranges for the range-input and shape checks. A range reference is not an
    # array to every Excel function: EDATE and EOMONTH refuse a multi-cell range that they
    # accept once it is calculated, so these cells hold plain values, not formulas.
    $d = { param($y, $m, $dd) (Get-Date -Year $y -Month $m -Day $dd).Date.ToOADate() }
    Set-Block 'AE1:AH1' @(, @(1, 2, 3, 4))
    Set-Block 'AE2:AH2' @(, @(10, 20, 30, 40))
    Set-Block 'AE3:AH3' @(, @((& $d 2026 6 30), (& $d 2026 7 1), (& $d 2026 7 15), (& $d 2026 9 1)))
    Set-Block 'AE4:AF4' @(, @((& $d 2026 7 1), (& $d 2026 8 1)))
    Set-Block 'AE5:AF5' @(, @((& $d 2026 7 31), (& $d 2026 8 31)))
    Set-Block 'AN1:AN3' @(@(1), @(2), @(3))
    Set-Block 'AP1:AP3' @(@(& $d 2026 6 30), @(& $d 2027 1 31), @(& $d 2028 2 29))
    Set-Block 'AL1:AL1' @(, @('TBA'))

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
    # A refused Quit() is reported; Wait-ExcelExit below then ends this script's Excel.
    if ($xl)  { try { $xl.Quit() } catch { Write-Output ('Excel refused Quit(): ' + $_.Exception.Message) } }
    # Every reference left keeps Excel running after Quit().
    foreach ($com in @($cell, $bad, $used, $ws, $tmp, $wb, $xl)) {
        if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
            [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($com)
        }
    }
    $com = $cell = $bad = $used = $ws = $tmp = $wb = $xl = $null
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
    Wait-ExcelExit $excel
}
exit $exit
