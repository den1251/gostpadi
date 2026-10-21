/* ЛБ3, задача 4: статистика массива.
 * Количество чётных и нечётных, среднее, замена отрицательных нулём. */
#include <stdio.h>
#include <stdlib.h>

#define N_MAX 100
#define LOW -100
#define HIGH 100

int main(void) {
    int a[N_MAX];
    int n = 0;
    int i = 0;
    int even = 0;
    int odd = 0;
    int sum = 0;

    while (1) {
        printf("n (1..%d): ", N_MAX);
        if (scanf("%d", &n) == 1 && n >= 1 && n <= N_MAX) {
            break;
        }
        printf("нужно 1..%d\n", N_MAX);
        while (getchar() != '\n') {
        }
    }
    srand(42);
    for (i = 0; i < n; i++) {
        a[i] = rand() % (HIGH - LOW + 1) + LOW;
    }
    for (i = 0; i < n; i++) {
        if (a[i] % 2 == 0) {
            even = even + 1;
        } else {
            odd = odd + 1;
        }
        if (a[i] < 0) {
            a[i] = 0;
        }
        sum = sum + a[i];
    }
    printf("массив после замены отрицательных:\n");
    for (i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    printf("чётных = %d, нечётных = %d\n", even, odd);
    printf("сумма = %d, среднее = %.2lf\n", sum, (double)sum / n);
    return 0;
}
