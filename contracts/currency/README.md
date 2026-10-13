# Fixed currency minor units

`minor-units.json` contains only alphabetic currency codes and numeric minor-unit
counts from the current SIX ISO 4217 List One snapshot. Repeated country entries
are deduplicated; N.A., XTS and XXX are excluded. Codes are case-sensitive.

`provenance.json` records the source URL, publication date, source checksum,
selection rule and exact table digest. That digest participates in model identity.
The SDK uses the bundled snapshot without network access; updates require an
explicit new snapshot and change model digests. This is not the ISO standard text.

Source attribution: SIX Financial Information AG, ISO 4217 Maintenance Agency.
[ISO's currency-code page](https://www.iso.org/iso-4217-currency-codes.html)
states that currency codes may be used free of charge. This attribution does not
assert an additional license grant or ISO/SIX endorsement of this SDK.

To reproduce, supply the exact archived XML identified by the source checksum:
`python tools/p0/freeze_currency.py --source <list-one.xml> --out contracts/currency`.
The updater fails on conflicting duplicates or unexpected minor-unit values.
