{{/*
Expand the name of the chart.
*/}}
{{- define "diaspor-api-server.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
Truncated at 63 chars because some Kubernetes name fields are limited to
that by the DNS RFC-1035 label spec.
*/}}
{{- define "diaspor-api-server.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Chart label — used in the standard Helm label set.
*/}}
{{- define "diaspor-api-server.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels applied to every rendered object.
*/}}
{{- define "diaspor-api-server.labels" -}}
helm.sh/chart: {{ include "diaspor-api-server.chart" . }}
{{ include "diaspor-api-server.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Selector labels — the subset used in Deployment/Service selectors.
These MUST be immutable across upgrades.
*/}}
{{- define "diaspor-api-server.selectorLabels" -}}
app.kubernetes.io/name: {{ include "diaspor-api-server.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Name of the ServiceAccount to use.
*/}}
{{- define "diaspor-api-server.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "diaspor-api-server.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Name of the Secret that holds DIASPOR_JWT_SECRET (and any sibling
secret env vars).
*/}}
{{- define "diaspor-api-server.secretName" -}}
{{- printf "%s-env" (include "diaspor-api-server.fullname" .) }}
{{- end }}
