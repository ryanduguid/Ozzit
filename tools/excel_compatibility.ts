// Run in Excel's Automate tab on a disposable copy of the hash-checked workbook.
interface ProbeCase {
  id: string;
  formula: string;
  expected: number[][];
}

interface ProbeResult {
  id: string;
  formula: string;
  actual: (string | number | boolean)[][];
  passed: boolean;
}

function matchesSpill(actual: (string | number | boolean)[][], expected: number[][]): boolean {
  const rows = actual.values();
  return actual.length === expected.length && expected.every(expectedRow => {
    const row = rows.next();
    if (row.done || row.value.length !== expectedRow.length) return false;
    const cells = row.value.values();
    return expectedRow.every(expectedValue => {
      const cell = cells.next();
      return !cell.done && typeof cell.value === "number" &&
        Math.abs(cell.value - expectedValue) <= 0.000001;
    });
  });
}

// eslint-disable-next-line no-unused-vars, @typescript-eslint/no-unused-vars -- Excel calls this Office Scripts entry point.
function main(workbook: ExcelScript.Workbook, host: string, sourceSha256: string): string {
  if (!host.trim() || !/^[a-f0-9]{64}$/i.test(sourceSha256)) {
    throw new Error("Supply the Excel host and build, and the SHA-256 of the file opened.");
  }
  // The Windows evaluator uses the same JSON cases. CI compares both copies.
  const probes: ProbeCase[] = [
    {"id":"rolling","formula":"=oz.RollingSumλ({1,2,3,4},2)","expected":[[1,3,5,7]]},
    {"id":"rolling-changed","formula":"=oz.RollingSumλ({2,4,6,8},2)","expected":[[2,6,10,14]]},
    {"id":"lease","formula":"=oz.LeaseScheduleλ({105},0.05)","expected":[[100],[105],[5],[0]]},
    {"id":"lease-changed","formula":"=oz.LeaseScheduleλ({210},0.05)","expected":[[200],[210],[10],[0]]}
  ];
  const app = workbook.getApplication();
  const sheet = workbook.addWorksheet();
  const results: ProbeResult[] = [];
  try {
    const anchor = sheet.getRange("A1");
    for (const probe of probes) {
      anchor.setFormula(probe.formula);
      app.calculate(ExcelScript.CalculationType.fullRebuild);
      const spill = anchor.getSpillingToRange();
      const actual = (spill || anchor).getValues();
      const passed = !!spill && matchesSpill(actual, probe.expected);
      results.push({ id: probe.id, formula: probe.formula, actual, passed });
    }
  } finally {
    sheet.delete();
  }
  const evidence = JSON.stringify({
    host_reported_by_operator: host,
    source_sha256_supplied_by_operator: sourceSha256.toLowerCase(),
    evaluated_at_utc: new Date().toISOString(),
    calculation_engine_version: app.getCalculationEngineVersion(),
    results,
    passed: results.every(result => result.passed)
  });
  console.log(evidence);
  return evidence;
}
