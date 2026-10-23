(* ALPHA-Ω alternate numerics kernel (Modula-2).
   Copyright (C) 2025-2026 ALPHA-Ω contributors
   SPDX-License-Identifier: AGPL-3.0-or-later

   Added 2026-10-23: an independent reimplementation of the signal
   processor's spectral kernel, for cross-checking the Rust engine's
   arithmetic in a second language and toolchain (gm2).

   Computes the Goertzel single-bin power at 10 Hz over one
   128-sample window of a pure 10 Hz sine (amplitude 1, 256 Hz
   sampling, no noise), converts the result to the engine's
   fixed-point scale (x 100000) using integer arithmetic only, and
   applies the elevation threshold (50000) as an integer comparison.
   The Rust reference value for this exact window is 100000
   (asserted by the engine test `processor_pure_sine_power`). *)

MODULE AlphaNumerics;

FROM STextIO IMPORT WriteString, WriteChar, WriteLn;
FROM LongMath IMPORT sin, cos;

CONST
  N        = 128;              (* window length, samples *)
  Rate     = 256.0;            (* sample rate, Hz *)
  TargetHz = 10.0;             (* Goertzel target frequency *)
  Scale    = 100000;           (* engine fixed-point scale *)
  Threshold = 50000;           (* engine POWER_THRESHOLD *)
  Pi       = 3.14159265358979323846;

VAR
  coeff, w, s, s1, s2, x, powerRaw, amp2 : LONGREAL;
  k, i : CARDINAL;
  scaled : LONGINT;
  buf : ARRAY [0..19] OF CHAR;
  nbuf, d : CARDINAL;

BEGIN
  (* Goertzel recurrence at bin k = round(TargetHz * N / Rate) = 5 *)
  k := VAL(CARDINAL, TargetHz * LFLOAT(N) / Rate + 0.5);
  w := 2.0 * Pi * LFLOAT(k) / LFLOAT(N);
  coeff := 2.0 * cos(w);
  s1 := 0.0; s2 := 0.0;
  FOR i := 0 TO N - 1 DO
    x := sin(2.0 * Pi * TargetHz * LFLOAT(i) / Rate);
    s := x + coeff * s1 - s2;
    s2 := s1;
    s1 := s;
  END;
  powerRaw := s1 * s1 + s2 * s2 - coeff * s1 * s2;
  amp2 := (2.0 / LFLOAT(N)) * (2.0 / LFLOAT(N)) * powerRaw;

  (* Fixed-point conversion by integer arithmetic (round half up). *)
  scaled := TRUNC(amp2 * LFLOAT(Scale) + 0.5);

  WriteString("ALPHA-NUMERICS amp2_fixed=");
  (* print scaled (LONGINT) digit by digit *)
  IF scaled < 0 THEN WriteChar('-'); scaled := -scaled; END;
  nbuf := 0;
  REPEAT
    buf[nbuf] := CHR(VAL(CARDINAL, scaled MOD 10) + ORD('0'));
    INC(nbuf);
    scaled := scaled DIV 10;
  UNTIL scaled = 0;
  WHILE nbuf > 0 DO
    DEC(nbuf);
    WriteChar(buf[nbuf]);
  END;
  WriteString(" elevated=");
  IF TRUNC(amp2 * LFLOAT(Scale) + 0.5) >= Threshold THEN
    WriteString("TRUE");
  ELSE
    WriteString("FALSE");
  END;
  WriteLn;
END AlphaNumerics.
