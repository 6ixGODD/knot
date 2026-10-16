package main

import (
	"bytes"
	"errors"
	"math/big"
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/dlclark/regexp2"
	"github.com/santhosh-tekuri/jsonschema/v6"
)

type offlineSchemas struct{}

type schemaRegexp struct{ *regexp2.Regexp }

func (r schemaRegexp) MatchString(s string) bool {
	matched, err := r.Regexp.MatchString(s)
	return err == nil && matched
}
func compileRegexp(pattern string) (jsonschema.Regexp, error) {
	r, err := regexp2.Compile(pattern, regexp2.ECMAScript)
	if err != nil {
		return nil, err
	}
	r.MatchTimeout = time.Second
	return schemaRegexp{r}, nil
}

func (offlineSchemas) Load(string) (any, error) { return nil, errors.New("external schema prohibited") }

func idFormat(value any) error {
	s, ok := value.(string)
	if !ok {
		return nil
	}
	if len(s) == 0 || len(s) > 128 || strings.IndexFunc(s, unicode.IsControl) >= 0 {
		return errors.New("invalid knot-id")
	}
	return nil
}

func decimalFormat(value any) error {
	s, ok := value.(string)
	if !ok {
		return nil
	}
	invalid := errors.New("invalid knot-decimal")
	if len(s) == 0 || len(s) > 128 {
		return invalid
	}
	unsigned := strings.TrimPrefix(s, "-")
	parts := strings.Split(unsigned, ".")
	if len(parts) > 2 {
		return invalid
	}
	for _, part := range parts {
		if part == "" {
			return invalid
		}
		for _, ch := range part {
			if ch < '0' || ch > '9' {
				return invalid
			}
		}
	}
	if len(parts) == 2 && len(strings.TrimRight(parts[1], "0")) > 9 {
		return invalid
	}
	number, ok := new(big.Rat).SetString(s)
	if !ok || number.Abs(number).Cmp(new(big.Rat).SetInt64(9007199254740991)) > 0 {
		return invalid
	}
	return nil
}

var responseSchemas = sync.OnceValues(func() (map[string]*jsonschema.Schema, error) {
	compiler := jsonschema.NewCompiler()
	compiler.UseRegexpEngine(compileRegexp)
	compiler.UseLoader(offlineSchemas{})
	compiler.AssertFormat()
	compiler.RegisterFormat(&jsonschema.Format{Name: "knot-id", Validate: idFormat})
	compiler.RegisterFormat(&jsonschema.Format{Name: "knot-decimal", Validate: decimalFormat})
	schemas := make(map[string]*jsonschema.Schema)
	for name, source := range map[string]string{"result": resultSchemaJSON, "error": errorSchemaJSON} {
		document, err := jsonschema.UnmarshalJSON(strings.NewReader(source))
		if err != nil {
			return nil, err
		}
		url := "https://schemas.invalid/" + name + ".json"
		if err = compiler.AddResource(url, document); err != nil {
			return nil, err
		}
		schemas[name], err = compiler.Compile(url)
		if err != nil {
			return nil, err
		}
	}
	return schemas, nil
})

func validateResponse(name string, raw []byte) error {
	if len(raw) == 0 || len(raw) > 4194304 || !utf8.Valid(raw) {
		return errors.New("invalid response bytes")
	}
	schemas, err := responseSchemas()
	if err != nil {
		return err
	}
	value, err := jsonschema.UnmarshalJSON(bytes.NewReader(raw))
	if err != nil {
		return err
	}
	return schemas[name].Validate(value)
}
