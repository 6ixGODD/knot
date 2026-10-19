package main

import (
	"mime"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestEmbeddedModulesUseJavaScriptMIME(t *testing.T) {
	handler, err := staticAssets()
	if err != nil {
		t.Fatal(err)
	}
	for _, file := range []string{"app.js", "session.mjs"} {
		t.Run(file, func(t *testing.T) {
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/"+file, nil))
			if response.Code != http.StatusOK {
				t.Fatalf("status=%d", response.Code)
			}
			mediaType, _, err := mime.ParseMediaType(response.Header().Get("Content-Type"))
			if err != nil || mediaType != "text/javascript" {
				t.Fatalf("module MIME=%q error=%v", mediaType, err)
			}
			if !strings.Contains(response.Body.String(), "createSession") {
				t.Fatal("module source missing")
			}
		})
	}
}
