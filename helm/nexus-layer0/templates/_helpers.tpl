
{{- define "nexus.fullname" -}}
{{- printf "%s-%s" .Release.Name "nexus-layer0" | trunc 63 | trimSuffix "-" -}}
{{- end -}}
