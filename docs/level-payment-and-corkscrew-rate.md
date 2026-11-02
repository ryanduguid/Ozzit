# Level payment and average-balance corkscrew

`oz.LevelPaymentλ` and `oz.CorkscrewRateλ` each solve in closed form what a worksheet
would otherwise solve with Goal Seek or a circular reference. This page derives both
results and records the checks that test them in Excel. Neither function makes a tax or
accounting classification; the reviewer decides how a model uses the results.

## LevelPaymentλ

### The loan

A loan of principal $P$ runs over $n$ periods. Period $k$ has its own rate $r_k$, such
as one row of `oz.DayCountRateλ()`, so a 31-day month accrues more than a 30-day one.
A level payment $X$ comes off at the end of each period, and the balance after the last
payment is the balloon $B$, 0 by default. Each rate must exceed $-1$, so that
$1 + r_k > 0$.

### Deriving the payment

The balance at the end of period $k$ is

$$C_k = C_{k-1}(1 + r_k) - X, \qquad C_0 = P.$$

Define the discount factor to the end of period $k$ as

$$D_k = \prod_{j=1}^{k} \frac{1}{1 + r_j}, \qquad D_0 = 1.$$

Multiplying the recurrence by $D_k$ gives $C_k D_k = C_{k-1} D_{k-1} - X D_k$, because
$D_k (1 + r_k) = D_{k-1}$. Summing from $k = 1$ to $n$ telescopes:

$$C_n D_n = P - X \sum_{k=1}^{n} D_k.$$

Setting $C_n = B$ and solving for $X$:

$$X = \frac{P - B\,D_n}{\sum_{k=1}^{n} D_k}.$$

The principal is the present value of the payments and the balloon, discounted through
each period's own rate. The function computes the factors with one `SCAN`,
`SCAN(1, Rates, LAMBDA(Acc, Rate, Acc / (1 + Rate)))`, and then the quotient.

### Special cases

- **One rate $r$ in every period.** $D_k = (1 + r)^{-k}$ and
  $\sum D_k = (1 - (1 + r)^{-n}) / r$, so $X = (P - B(1 + r)^{-n})\,r / (1 - (1 + r)^{-n})$.
  Excel's `PMT(rate, nper, pv, fv)` solves $pv(1 + r)^n + pmt\,((1 + r)^n - 1)/r + fv = 0$;
  with $pv = P$ and $fv = -B$ that is the same equation, so
  $X = -\mathrm{PMT}(r, n, P, -B)$.
- **Nil rates.** Every $D_k = 1$ and $X = (P - B)/n$.
- **Worked example.** $P = 1000$ over 3 periods at 1%: $\sum D_k = 2.940985$, so
  $X = 340.02$, the result the help prints.

## CorkscrewRateλ

### The corkscrew

A corkscrew opens each period on $O$, adds its flows $F$ (the sum of the flow rows in that
column) and interest $I$, and closes on $C$. Interest is the period's rate $r$ times the
average of the opening and closing balances, and it is added to the balance:

$$I = r\,\frac{O + C}{2}, \qquad C = O + F + I.$$

$C$ depends on $I$ and $I$ on $C$: on a worksheet that is a circular reference.

### Deriving the closing balance

Substituting $C$ into $I$:

$$I = \frac{r}{2}(2O + F + I) \quad\Rightarrow\quad I\left(1 - \frac{r}{2}\right) = \frac{r}{2}(2O + F) \quad\Rightarrow\quad I = \frac{r\,(2O + F)}{2 - r}.$$

So

$$C = O + F + \frac{r\,(2O + F)}{2 - r} = \frac{O\,(2 + r) + 2F}{2 - r}.$$

The closing balance is the only state carried to the next period, whose opening it is,
so one `SCAN` over the columns produces every closing balance. The function then reads
the openings off the closings and computes interest from its definition,
$r(O + C)/2$, not from the closing balance less the other rows. The check rows below
therefore test that the 2 rows agree.

The rate must be below 2, where $2 - r$ reaches nought. It must also be above $-1$, as
for `oz.LevelPaymentλ`; the derivation itself needs only $r < 2$, so the lower bound is
a choice for review.

### Worked example

Opening 1,000, rate 1%, flows of 100 in 2 periods. Period 1:
$C = (1000 \times 2.01 + 200)/1.99 = 1110.55$ and $I = 10.55$. Period 2 opens on
1,110.55, closes on 1,222.22 and earns 11.66. The help prints these 4 rows.

## Checks

The native self-test (`tools/excel_selftest.ps1`) carries 39 hand-written assertions for
the 2 functions, and `ozzit generate_selftest_examples` adds 5 more from their help. On
1 November 2026, Excel 16.0 build 20527 ran all 1,271 assertions in the library with
0 failures, recalculated 1,129 formulas with 0 in error, and `ozzit verify_cache`
found every cached value equal to what its formula produces.

`oz.LevelPaymentλ`:

- Equals `-PMT()` for 360 monthly periods at 0.5%, and with a 20,000 balloon over 60
  periods at 0.4%, within 1e-7.
- Under Actual/Actual day counts over 24 months from 1 July 2026 at 7.3%, the loan run
  forward period by period, $C_k = C_{k-1}(1 + r_k) - X$, closes on nil, and on the
  balloon when one is given, within 1e-7. That recurrence is not the discount-factor sum
  the function evaluates.
- Nil rates divide the principal less the balloon equally; a blank balloon cell is no
  balloon.
- Refused with a message: a rate of -1, a column of rates, text principal, a logical
  balloon, blank rate cells, a single blank rate cell and a principal of more than one
  cell. An error in Rates shows.

`oz.CorkscrewRateλ`, on 24 months of the same day-count rates with 2 flow rows:

- Check row: in every column, opening plus flows plus interest equals closing.
- Check row: every opening equals the previous closing.
- An independent solve, which iterates $I \leftarrow r(2O + F + I)/2$ 60 times for each
  period instead of using the closed form, gives the same closing balances within 1e-7.
- The first interest of the worked example equals $2210/1.99 - 1100$; one rate equals the
  same rate given for every period; a nil rate gives `oz.Corkscrewλ`'s closing row; a
  single period works; worksheet ranges give the same result as arrays.
- Refused with a message: a rate of 2, a rate row of the wrong length, a text rate, a
  column of rates, a rate of -1, a blank rate cell and a row with a blank rate cell; an
  Opening of text, `TRUE` or more than one cell; and a flow of text or `TRUE`, which
  `SUM` would leave out of the closing balance whilst the flows row still showed it. An
  error in Rates or in a flow shows; a blank Opening and a `""` flow are nil.
- Each refusal check requires the function's exact message alone in one cell, so
  returning the help, a schedule or another message fails it.

On 1 November 2026 the self-test also ran against copies of the workbook with each
function deliberately broken, and failed every time:

- blank rates read as nil: the blank-rate refusals of each function failed;
- every refusal returning the help: all 20 refusal checks failed, those for
  `oz.CorkScrewReversalλ` included;
- discount factors of $1 - r$ in place of $1/(1 + r)$, and interest on the opening
  balance in place of the average: the PMT, forward-run, column, iterated-solve and
  hand-worked checks failed.

No check yet compares either function with an outside calculator or a published loan
schedule.

## Decisions for review

1. `oz.LevelPaymentλ` pays in arrears only. PMT's `type = 1`, payment at the start of each
   period, is not offered.
2. The payment is positive for a positive principal, unlike `PMT()`, so that it reads as
   the amount the borrower pays.
3. `oz.CorkscrewRateλ` adds interest to the balance. Interest paid in cash each period,
   which leaves the balance alone, is not offered; there the closing balance does not
   depend on the interest, so no closed form is needed.
4. Average-balance interest assumes the period's flows arrive evenly through it.
5. Rates must lie above -1, and below 2 for `oz.CorkscrewRateλ`, whose Opening must be a
   single number and whose flows must be numbers or blank.
6. Both functions sit under a new LOANS heading in `oz.AboutFinancialλ`.
